//! The Reader (Layer 2): builds the [`Parsed`] datum tree on top of the Lexer.
//!
//! Fault-tolerant with top-level resync (ADR-0004): a malformed form never
//! panics and never loses the rest of the file.

use crate::datum::{Datum, DatumKind, Delim, Notation, Prefix};
use crate::error::ParseError;
use crate::lexer::Lexer;
use crate::options::Options;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// The result of reading a source string. Borrows the source (ADR-0008).
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed<'a> {
    /// A leading dialect directive such as Racket's `#lang racket`, if any.
    /// Passive — captured, not acted on (ADR-0012). Always `None` for Scheme.
    pub lang_line: Option<&'a str>,
    /// Top-level forms, in source order.
    pub data: Vec<Datum<'a>>,
    /// Diagnostics from fault-tolerant recovery.
    pub errors: Vec<ParseError>,
}

/// Parse `source` under `options` into a datum tree. Never panics.
pub fn parse<'a>(source: &'a str, options: &Options) -> Parsed<'a> {
    let mut lang_line: Option<&'a str> = None;
    let tokens: Vec<Token> = Lexer::new(source, options)
        .filter(|t| {
            if t.kind == TokenKind::LangLine && lang_line.is_none() {
                // Capture the language spec after `#lang`, verbatim (ADR-0012).
                lang_line = Some(t.span.text(source).trim_start_matches("#lang").trim());
            }
            !matches!(
                t.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment
                    | TokenKind::BlockComment
                    | TokenKind::LangLine
            )
        })
        .collect();

    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        line_starts: line_starts(source),
        errors: Vec::new(),
        keyword_colon: options.keyword_colon,
        hash_keyword: options.hash_keyword,
        dotted_pairs: options.dotted_pairs,
        feature_conditional: options.feature_conditional,
    };

    let data = parser.parse_top_level();
    Parsed {
        lang_line,
        data,
        errors: parser.errors,
    }
}

/// Convenience: iterate top-level data, discarding diagnostics.
pub fn read_all<'a>(source: &'a str, options: &Options) -> std::vec::IntoIter<Datum<'a>> {
    parse(source, options).data.into_iter()
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    line_starts: Vec<u32>,
    errors: Vec<ParseError>,
    keyword_colon: bool,
    hash_keyword: bool,
    dotted_pairs: bool,
    feature_conditional: bool,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).copied();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn text(&self, span: Span) -> &'a str {
        span.text(self.source)
    }

    fn line_of(&self, offset: u32) -> u32 {
        // Number of line starts <= offset == 1-based line.
        self.line_starts.partition_point(|&s| s <= offset) as u32
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        let line = self.line_of(span.start);
        self.errors.push(ParseError {
            span,
            line,
            message: message.into(),
        });
    }

    fn parse_top_level(&mut self) -> Vec<Datum<'a>> {
        let mut data = Vec::new();
        while let Some(t) = self.peek() {
            match t.kind {
                TokenKind::Close(_) => {
                    self.advance();
                    self.error(t.span, "unexpected closing delimiter");
                }
                _ => {
                    if let Some(d) = self.parse_datum() {
                        data.push(d);
                    } else {
                        break;
                    }
                }
            }
        }
        data
    }

    /// Parse one datum. Skips `#;`-discarded data. Returns `None` at EOF or when
    /// the next token is a closing delimiter (left for the caller).
    fn parse_datum(&mut self) -> Option<Datum<'a>> {
        loop {
            let t = self.peek()?;
            match t.kind {
                TokenKind::Close(_) => return None,
                TokenKind::Prefix(Prefix::Discard) => {
                    self.advance();
                    // Drop the next datum entirely.
                    let _ = self.parse_datum();
                    continue;
                }
                TokenKind::Error => {
                    self.advance();
                    self.error(t.span, "malformed token");
                    continue;
                }
                _ => break,
            }
        }

        let t = self.advance()?;
        let line = self.line_of(t.span.start);
        let kind = match t.kind {
            TokenKind::Open(delim) => return Some(self.finish_list(delim, t.span)),
            TokenKind::HashOpen(delim) => return Some(self.finish_hash(delim, t.span)),
            TokenKind::Str => DatumKind::Str(self.text(t.span)),
            TokenKind::Char => DatumKind::Char(self.text(t.span)),
            TokenKind::Bool(b) => DatumKind::Bool(b),
            TokenKind::Atom => {
                classify_atom(self.text(t.span), self.keyword_colon, self.hash_keyword)
            }
            TokenKind::HashTag => {
                // `#tag <form>`: attach the tag to the following datum.
                let tag = &self.text(t.span)[1..]; // drop leading '#'
                let inner = match self.parse_datum() {
                    Some(d) => Some(Box::new(d)),
                    None => {
                        self.error(t.span, "tagged literal with no following datum");
                        None
                    }
                };
                let end = inner.as_ref().map(|d| d.span.end).unwrap_or(t.span.end);
                return Some(Datum {
                    kind: DatumKind::HashLiteral { tag, inner },
                    span: Span::new(t.span.start, end),
                    line,
                });
            }
            TokenKind::Prefix(Prefix::ReaderConditional(sense)) if self.feature_conditional => {
                // Common Lisp `#+feature form` / `#-feature form`: read the
                // feature test, then the guarded form. First cut: the feature
                // test is consumed but not retained; the guarded form is kept so
                // structure stays correct (one form).
                let _feature = self.parse_datum();
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(t.span, "reader conditional with no guarded form");
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Prefixed {
                        prefix: Prefix::ReaderConditional(sense),
                        notation: Notation::Shorthand,
                        inner: Box::new(inner),
                    },
                    span,
                    line,
                });
            }
            TokenKind::Prefix(Prefix::Meta) => {
                // `^meta target` / `#^meta target`: read the metadata form, then
                // the target it annotates. First cut: the metadata is consumed
                // but not retained; the target is returned wrapped (structure is
                // correct — one form — but the metadata content is dropped).
                let _meta = self.parse_datum();
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(t.span, "metadata with no target datum");
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Prefixed {
                        prefix: Prefix::Meta,
                        notation: Notation::Shorthand,
                        inner: Box::new(inner),
                    },
                    span,
                    line,
                });
            }
            TokenKind::Prefix(prefix) => {
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(t.span, "prefix with no following datum");
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Prefixed {
                        prefix,
                        notation: Notation::Shorthand,
                        inner: Box::new(inner),
                    },
                    span,
                    line,
                });
            }
            TokenKind::Label => {
                let id = label_id(self.text(t.span));
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(t.span, "datum label with no following datum");
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Label {
                        id,
                        inner: Box::new(inner),
                    },
                    span,
                    line,
                });
            }
            TokenKind::LabelRef => DatumKind::LabelRef {
                id: label_id(self.text(t.span)),
            },
            // Unreachable: filtered out or handled above.
            TokenKind::Whitespace
            | TokenKind::LineComment
            | TokenKind::BlockComment
            | TokenKind::LangLine
            | TokenKind::Close(_)
            | TokenKind::Error => return None,
        };
        Some(Datum {
            kind,
            span: t.span,
            line,
        })
    }

    /// Read list items until the matching close, then apply longhand-quote
    /// folding (ADR-0002). `open` is the already-consumed opening token span.
    fn finish_list(&mut self, delim: Delim, open: Span) -> Datum<'a> {
        let line = self.line_of(open.start);
        let mut items: Vec<Datum<'a>> = Vec::new();
        let mut tail: Option<Box<Datum<'a>>> = None;
        let end;

        loop {
            let Some(t) = self.peek() else {
                self.error(open, "unclosed list");
                end = items.last().map(|d| d.span.end).unwrap_or(open.end);
                break;
            };

            match t.kind {
                TokenKind::Close(close_delim) => {
                    self.advance();
                    if !close_matches(delim, close_delim) {
                        self.error(t.span, "mismatched closing delimiter");
                    }
                    end = t.span.end;
                    break;
                }
                TokenKind::Atom
                    if self.dotted_pairs
                        && tail.is_none()
                        && !items.is_empty()
                        && self.text(t.span) == "." =>
                {
                    self.advance(); // consume the dot
                    match self.parse_datum() {
                        Some(d) => tail = Some(Box::new(d)),
                        None => self.error(t.span, "dotted list with no tail datum"),
                    }
                    // The loop continues; the next token should be the close.
                }
                _ => match self.parse_datum() {
                    Some(d) => items.push(d),
                    None => {
                        // A stray close was seen; loop will consume it.
                        if !matches!(self.peek().map(|t| t.kind), Some(TokenKind::Close(_))) {
                            self.error(open, "unclosed list");
                            end = open.end;
                            break;
                        }
                    }
                },
            }
        }

        let datum = Datum {
            kind: DatumKind::List { delim, items, tail },
            span: Span::new(open.start, end),
            line,
        };
        fold_longhand(datum)
    }

    /// Read a `#(`-style hash literal: items until the matching close, wrapped
    /// as a [`DatumKind::HashLiteral`].
    fn finish_hash(&mut self, delim: Delim, open: Span) -> Datum<'a> {
        let line = self.line_of(open.start);
        // tag = text between '#' and the opening delimiter char.
        let tag = &self.source[open.start as usize + 1..open.end as usize - 1];

        let inner_open = Span::new(open.end - 1, open.end); // the '(' itself
        let inner = self.finish_list(delim, inner_open);
        let span = Span::new(open.start, inner.span.end);
        Datum {
            kind: DatumKind::HashLiteral {
                tag,
                inner: Some(Box::new(inner)),
            },
            span,
            line,
        }
    }
}

/// Fold `(quote x)` and friends into a longhand [`DatumKind::Prefixed`]
/// (ADR-0002). Only the exact two-element round-list shape qualifies.
fn fold_longhand(datum: Datum<'_>) -> Datum<'_> {
    match datum.kind {
        DatumKind::List {
            delim: Delim::Round,
            mut items,
            tail: None,
        } if items.len() == 2 => {
            if let DatumKind::Symbol(s) = items[0].kind {
                if let Some(prefix) = quote_symbol(s) {
                    let inner = items.pop().unwrap(); // items[1]
                    return Datum {
                        kind: DatumKind::Prefixed {
                            prefix,
                            notation: Notation::Longhand,
                            inner: Box::new(inner),
                        },
                        span: datum.span,
                        line: datum.line,
                    };
                }
            }
            Datum {
                kind: DatumKind::List {
                    delim: Delim::Round,
                    items,
                    tail: None,
                },
                span: datum.span,
                line: datum.line,
            }
        }
        other => Datum {
            kind: other,
            span: datum.span,
            line: datum.line,
        },
    }
}

fn quote_symbol(s: &str) -> Option<Prefix> {
    match s {
        "quote" => Some(Prefix::Quote),
        "quasiquote" => Some(Prefix::Quasiquote),
        "unquote" => Some(Prefix::Unquote),
        "unquote-splicing" => Some(Prefix::UnquoteSplicing),
        _ => None,
    }
}

/// Extract the numeric id from a `#n=` / `#n#` label token's text.
fn label_id(text: &str) -> &str {
    &text[1..text.len() - 1]
}

/// Whether a close delimiter closes an open one. A set `#{ ... }` is closed by
/// a curly `}`.
fn close_matches(open: Delim, close: Delim) -> bool {
    match open {
        Delim::Set => close == Delim::Curly,
        other => close == other,
    }
}

fn classify_atom(text: &str, keyword_colon: bool, hash_keyword: bool) -> DatumKind<'_> {
    if hash_keyword && text.starts_with("#:") {
        return DatumKind::Keyword(text);
    }
    if keyword_colon && text.starts_with(':') {
        return DatumKind::Keyword(text);
    }
    if looks_like_number(text) {
        DatumKind::Number(text)
    } else {
        DatumKind::Symbol(text)
    }
}

/// Coarse "is this a number in Scheme" check (ADR: value never interpreted).
/// Deliberately conservative — ambiguous atoms fall back to `Symbol`.
fn looks_like_number(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() {
        return false;
    }
    // Clojure symbolic values: ##Inf, ##-Inf, ##NaN.
    if s.starts_with("##") {
        return true;
    }
    // Radix / exactness prefix: #x, #b, #e, ...
    if b[0] == b'#' {
        return b.len() >= 2
            && matches!(
                b[1].to_ascii_lowercase(),
                b'e' | b'i' | b'b' | b'o' | b'd' | b'x'
            );
    }
    let mut i = 0;
    if b[0] == b'+' || b[0] == b'-' {
        i = 1;
    }
    if i >= b.len() {
        return false; // lone + or -
    }
    if b[i].is_ascii_digit() {
        return true;
    }
    // .5 style
    b[i] == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()
}

/// Byte offsets of the start of each line (line 1 begins at offset 0).
fn line_starts(source: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i as u32 + 1);
        }
    }
    starts
}
