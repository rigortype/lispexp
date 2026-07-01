//! The Reader (Layer 2): builds the [`Parsed`] datum tree on top of the Lexer.
//!
//! Fault-tolerant with top-level resync (ADR-0004): a malformed form never
//! panics and never loses the rest of the file.

use crate::datum::{Datum, DatumKind, Delim, Notation, Prefix};
use crate::error::{ErrorKind, ParseError};
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
    let tokens = significant_tokens(source, options, Some(&mut lang_line));
    let mut parser = Parser::new(source, tokens, options);
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

/// One top-level form read at or after a byte offset (ADR-0023).
///
/// The result of [`parse_form_at`]. Spans are absolute into the original
/// `source`.
#[derive(Debug, Clone, PartialEq)]
pub struct FormAt<'a> {
    /// The form that was read.
    pub form: Datum<'a>,
    /// Diagnostics produced while reading just this form.
    pub errors: Vec<ParseError>,
    /// Byte offset just past the form — the offset a consumer passes back to
    /// read the following form. A convenience alias for `form.span.end`; it
    /// never includes trailing trivia.
    pub end: u32,
}

/// Read exactly one top-level form at or after byte offset `start` (ADR-0023).
///
/// Returns `None` if there is no further form (only whitespace/comments, or
/// `start` is past end of input). Spans are absolute into `source`. Because
/// recovery is top-level-granular (ADR-0004), a consumer can re-validate just
/// the form(s) an edit falls in and compare their small [`ErrorKind`] sets
/// locally — the "reject only newly-introduced errors" policy stays with the
/// consumer.
///
/// **Precondition:** `start` must sit at or before a top-level form boundary
/// (obtain boundaries from a prior [`parse`]'s spans, or feed [`FormAt::end`]
/// back). An offset strictly inside a form reads the next *inner* datum as if
/// it were top-level and may report spurious delimiter diagnostics.
///
/// A leading `#lang` line is skipped, not surfaced — use [`parse`] to capture
/// it.
pub fn parse_form_at<'a>(source: &'a str, start: u32, options: &Options) -> Option<FormAt<'a>> {
    let tokens = significant_tokens(source, options, None);
    let mut parser = Parser::new(source, tokens, options);
    // Skip to the first significant token at or after `start`.
    parser.pos = parser.tokens.partition_point(|t| t.span.start < start);

    // Consume any stray closing delimiters, then read one form.
    let form = loop {
        let t = parser.peek()?;
        if let TokenKind::Close(found) = t.kind {
            parser.advance();
            parser.error(t.span, ErrorKind::UnexpectedDelimiter { found });
            continue;
        }
        break parser.parse_datum()?;
    };
    let end = form.span.end;
    Some(FormAt {
        form,
        errors: parser.errors,
        end,
    })
}

/// Lex `source` and drop insignificant tokens (whitespace, comments, lang
/// line). If `capture_lang` is `Some`, the first `#lang` spec is captured into
/// it, verbatim (ADR-0012).
fn significant_tokens<'a>(
    source: &'a str,
    options: &Options,
    mut capture_lang: Option<&mut Option<&'a str>>,
) -> Vec<Token> {
    Lexer::new(source, options)
        .filter(|t| {
            if t.kind == TokenKind::LangLine {
                if let Some(slot) = capture_lang.as_deref_mut() {
                    if slot.is_none() {
                        *slot = Some(t.span.text(source).trim_start_matches("#lang").trim());
                    }
                }
            }
            !matches!(
                t.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment
                    | TokenKind::BlockComment
                    | TokenKind::LangLine
            )
        })
        .collect()
}

struct Parser<'a, 'o> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    line_starts: Vec<u32>,
    errors: Vec<ParseError>,
    /// Borrowed only for the duration of parsing (a separate lifetime from the
    /// source `'a`, so a caller's temporary `&Options` stays ergonomic).
    opts: &'o Options,
}

impl<'a, 'o> Parser<'a, 'o> {
    fn new(source: &'a str, tokens: Vec<Token>, options: &'o Options) -> Self {
        Parser {
            source,
            tokens,
            pos: 0,
            line_starts: line_starts(source),
            errors: Vec::new(),
            opts: options,
        }
    }

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

    fn error(&mut self, span: Span, kind: ErrorKind) {
        let line = self.line_of(span.start);
        self.errors.push(ParseError { span, line, kind });
    }

    fn parse_top_level(&mut self) -> Vec<Datum<'a>> {
        let mut data = Vec::new();
        while let Some(t) = self.peek() {
            match t.kind {
                TokenKind::Close(found) => {
                    self.advance();
                    self.error(t.span, ErrorKind::UnexpectedDelimiter { found });
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
                    self.error(
                        t.span,
                        ErrorKind::MalformedToken {
                            text: self.text(t.span).into(),
                        },
                    );
                    continue;
                }
                _ => break,
            }
        }

        let t = self.advance()?;
        let line = self.line_of(t.span.start);
        let kind = match t.kind {
            TokenKind::Open(delim) => return Some(self.finish_list(delim, t.span, true)),
            TokenKind::HashOpen(delim) => return Some(self.finish_hash(delim, t.span)),
            TokenKind::Str => DatumKind::Str(self.text(t.span)),
            TokenKind::Char => DatumKind::Char(self.text(t.span)),
            TokenKind::Bool(b) => DatumKind::Bool(b),
            TokenKind::Atom => classify_atom(self.text(t.span), self.opts),
            TokenKind::HashTag => {
                // `#tag <form>`: attach the tag to the following datum.
                let tag = &self.text(t.span)[1..]; // drop leading '#'
                let inner = match self.parse_datum() {
                    Some(d) => Some(Box::new(d)),
                    None => {
                        self.error(t.span, ErrorKind::DanglingTag);
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
            TokenKind::Prefix(prefix @ Prefix::FeatureConditional { .. })
                if self.opts.feature_conditional =>
            {
                // Common Lisp / Emacs Lisp `#+feature form` / `#-feature form`:
                // read the feature test (retained as `arg`), then the guarded
                // form (`inner`).
                let feature = self.parse_datum().map(Box::new);
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(t.span, ErrorKind::DanglingPrefix { prefix });
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Prefixed {
                        prefix,
                        notation: Notation::Shorthand,
                        inner: Box::new(inner),
                        arg: feature,
                    },
                    span,
                    line,
                });
            }
            TokenKind::Prefix(Prefix::Meta) => {
                // `^meta target` / `#^meta target`: read the metadata form
                // (retained as `arg`), then the target it annotates (`inner`).
                let meta = self.parse_datum().map(Box::new);
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(
                            t.span,
                            ErrorKind::DanglingPrefix {
                                prefix: Prefix::Meta,
                            },
                        );
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Prefixed {
                        prefix: Prefix::Meta,
                        notation: Notation::Shorthand,
                        inner: Box::new(inner),
                        arg: meta,
                    },
                    span,
                    line,
                });
            }
            TokenKind::Prefix(prefix) => {
                let inner = match self.parse_datum() {
                    Some(d) => d,
                    None => {
                        self.error(t.span, ErrorKind::DanglingPrefix { prefix });
                        return None;
                    }
                };
                let span = Span::new(t.span.start, inner.span.end);
                return Some(Datum {
                    kind: DatumKind::Prefixed {
                        prefix,
                        notation: Notation::Shorthand,
                        inner: Box::new(inner),
                        arg: None,
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
                        self.error(t.span, ErrorKind::DanglingLabel);
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
    /// folding (ADR-0002) when `fold` is set and the dialect enables it. `open`
    /// is the already-consumed opening token span. `fold` is `false` for a hash
    /// literal's inner list (its contents are data — `#(quote x)` is a vector,
    /// not a folded quote).
    fn finish_list(&mut self, delim: Delim, open: Span, fold: bool) -> Datum<'a> {
        let line = self.line_of(open.start);
        let mut items: Vec<Datum<'a>> = Vec::new();
        let mut tail: Option<Box<Datum<'a>>> = None;
        let end;

        loop {
            let Some(t) = self.peek() else {
                self.error(open, ErrorKind::UnclosedList { open: delim });
                end = items.last().map(|d| d.span.end).unwrap_or(open.end);
                break;
            };

            match t.kind {
                TokenKind::Close(close_delim) => {
                    self.advance();
                    if !close_matches(delim, close_delim) {
                        self.error(
                            t.span,
                            ErrorKind::MismatchedDelimiter {
                                expected: delim,
                                found: close_delim,
                            },
                        );
                    }
                    end = t.span.end;
                    break;
                }
                TokenKind::Atom
                    if self.opts.dotted_pairs
                        && tail.is_none()
                        && !items.is_empty()
                        && self.text(t.span) == "." =>
                {
                    self.advance(); // consume the dot
                    match self.parse_datum() {
                        Some(d) => tail = Some(Box::new(d)),
                        None => self.error(t.span, ErrorKind::DanglingDot),
                    }
                    // The loop continues; the next token should be the close.
                }
                _ => match self.parse_datum() {
                    Some(d) => items.push(d),
                    None => {
                        // A stray close was seen; loop will consume it.
                        if !matches!(self.peek().map(|t| t.kind), Some(TokenKind::Close(_))) {
                            self.error(open, ErrorKind::UnclosedList { open: delim });
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
        if fold && self.opts.fold_longhand {
            fold_longhand(datum, self.opts)
        } else {
            datum
        }
    }

    /// Read a `#(`-style hash literal: items until the matching close, wrapped
    /// as a [`DatumKind::HashLiteral`].
    fn finish_hash(&mut self, delim: Delim, open: Span) -> Datum<'a> {
        let line = self.line_of(open.start);
        // tag = text between '#' and the opening delimiter char.
        let tag = &self.source[open.start as usize + 1..open.end as usize - 1];

        let inner_open = Span::new(open.end - 1, open.end); // the '(' itself
                                                            // A hash literal's inner list is data: do not fold `#(quote x)` into a
                                                            // quote — it is a two-element vector.
        let inner = self.finish_list(delim, inner_open, false);
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
/// (ADR-0002). Only the exact two-element round-list shape qualifies, and only
/// when the head names a quote-family form whose shorthand glyph the dialect
/// actually has (`quote` iff `options.quote.is_some()`, etc.). Case-insensitive
/// dialects (`options.fold_case_insensitive`) match `(QUOTE X)` too.
fn fold_longhand<'a>(datum: Datum<'a>, opts: &Options) -> Datum<'a> {
    match datum.kind {
        DatumKind::List {
            delim: Delim::Round,
            mut items,
            tail: None,
        } if items.len() == 2 => {
            if let DatumKind::Symbol(s) = items[0].kind {
                if let Some(prefix) = quote_symbol(s, opts) {
                    let inner = items.pop().unwrap(); // items[1]
                    return Datum {
                        kind: DatumKind::Prefixed {
                            prefix,
                            notation: Notation::Longhand,
                            inner: Box::new(inner),
                            arg: None,
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

/// Map a longhand head symbol to the quote-family prefix it names, honoring the
/// dialect's glyph gates and case sensitivity. `quote`/`quasiquote` require the
/// corresponding shorthand glyph; `unquote`/`unquote-splicing` both require the
/// unquote glyph (e.g. AutoLISP has `quote` but no quasiquote/unquote).
fn quote_symbol(s: &str, opts: &Options) -> Option<Prefix> {
    let eq = |name: &str| {
        if opts.fold_case_insensitive {
            s.eq_ignore_ascii_case(name)
        } else {
            s == name
        }
    };
    if opts.quote.is_some() && eq("quote") {
        return Some(Prefix::Quote);
    }
    if opts.quasiquote.is_some() && eq("quasiquote") {
        return Some(Prefix::Quasiquote);
    }
    if opts.unquote.is_some() {
        if eq("unquote") {
            return Some(Prefix::Unquote);
        }
        if eq("unquote-splicing") {
            return Some(Prefix::UnquoteSplicing);
        }
    }
    None
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

fn classify_atom<'a>(text: &'a str, opts: &Options) -> DatumKind<'a> {
    if opts.hash_keyword && text.starts_with("#:") {
        return DatumKind::Keyword(text);
    }
    if opts.keyword_colon && text.starts_with(':') {
        return DatumKind::Keyword(text);
    }
    if looks_like_number(text) {
        return DatumKind::Number(text);
    }
    // Gambit/Gerbil trailing-colon keyword `foo:` (DSSSL/SRFI-88): an
    // *identifier* followed by `:`. Checked after `looks_like_number` so a
    // numeric-looking atom (`1:`, `#xFF:`) stays a Number as under strict R7RS;
    // a bare `:` is an ordinary symbol, so require a char before the colon.
    if opts.keyword_trailing_colon && text.len() > 1 && text.ends_with(':') {
        return DatumKind::Keyword(text);
    }
    DatumKind::Symbol(text)
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
