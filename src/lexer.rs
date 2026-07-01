//! The Lexer (Layer 1): source → linear token stream that tiles the input.
//!
//! Robust to incomplete input at character granularity; it never does the
//! top-level resync that is a Reader policy (ADR-0015). So far it covers the
//! Scheme lexical surface, driven by [`Options`].

use crate::datum::{Delim, Prefix};
use crate::options::Options;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// Lex `source` under `options`, yielding a token stream that tiles the input.
pub fn lex<'a>(source: &'a str, options: &'a Options) -> Lexer<'a> {
    Lexer::new(source, options)
}

/// The lexer. Implements [`Iterator`] over [`Token`]s.
pub struct Lexer<'a> {
    src: &'a str,
    opts: &'a Options,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, opts: &'a Options) -> Self {
        Lexer { src, opts, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn square_active(&self) -> bool {
        self.opts.square.is_delimiter()
    }

    fn curly_active(&self) -> bool {
        self.opts.curly.is_delimiter()
    }

    /// Does `c` end an atom / not belong to a symbol?
    fn is_terminator(&self, c: char) -> bool {
        c.is_whitespace()
            || c == '('
            || c == ')'
            || c == '"'
            || c == self.opts.line_comment
            || (self.square_active() && (c == '[' || c == ']'))
            || (self.curly_active() && (c == '{' || c == '}'))
    }

    fn token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            span: Span::new(start as u32, self.pos as u32),
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        let start = self.pos;
        let c = self.peek()?;

        // Whitespace run.
        if c.is_whitespace() {
            while matches!(self.peek(), Some(c) if c.is_whitespace()) {
                self.bump();
            }
            return Some(self.token(TokenKind::Whitespace, start));
        }

        // Line comment.
        if c == self.opts.line_comment {
            while !matches!(self.peek(), Some('\n') | None) {
                self.bump();
            }
            return Some(self.token(TokenKind::LineComment, start));
        }

        // Delimiters.
        match c {
            '(' => {
                self.bump();
                return Some(self.token(TokenKind::Open(Delim::Round), start));
            }
            ')' => {
                self.bump();
                return Some(self.token(TokenKind::Close(Delim::Round), start));
            }
            '[' if self.square_active() => {
                self.bump();
                return Some(self.token(TokenKind::Open(Delim::Square), start));
            }
            ']' if self.square_active() => {
                self.bump();
                return Some(self.token(TokenKind::Close(Delim::Square), start));
            }
            '{' if self.curly_active() => {
                self.bump();
                return Some(self.token(TokenKind::Open(Delim::Curly), start));
            }
            '}' if self.curly_active() => {
                self.bump();
                return Some(self.token(TokenKind::Close(Delim::Curly), start));
            }
            '"' => return Some(self.lex_string(start)),
            '|' if self.opts.piped_symbols => return Some(self.lex_piped_symbol(start)),
            _ => {}
        }

        // Hash-led reader syntax.
        if c == '#' && self.opts.hash_syntax {
            return Some(self.lex_hash(start));
        }

        // Quote family (per-dialect glyphs).
        if let Some(kind) = self.try_quote_prefix() {
            return Some(self.token(kind, start));
        }

        // Otherwise, an atom (symbol or number).
        self.bump();
        while let Some(c) = self.peek() {
            if self.is_terminator(c) {
                break;
            }
            self.bump();
        }
        Some(self.token(TokenKind::Atom, start))
    }

    fn try_quote_prefix(&mut self) -> Option<TokenKind> {
        let c = self.peek()?;
        if Some(c) == self.opts.quote {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::Quote));
        }
        if Some(c) == self.opts.quasiquote {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::Quasiquote));
        }
        if Some(c) == self.opts.unquote {
            self.bump();
            if self.peek() == Some(self.opts.splicing_suffix) {
                self.bump();
                return Some(TokenKind::Prefix(Prefix::UnquoteSplicing));
            }
            return Some(TokenKind::Prefix(Prefix::Unquote));
        }
        None
    }

    fn lex_string(&mut self, start: usize) -> Token {
        self.bump(); // opening quote
        loop {
            match self.bump() {
                Some('"') => return self.token(TokenKind::Str, start),
                Some('\\') => {
                    self.bump(); // escaped char
                }
                Some(_) => {}
                None => return self.token(TokenKind::Error, start), // unterminated
            }
        }
    }

    fn lex_piped_symbol(&mut self, start: usize) -> Token {
        self.bump(); // opening bar
        loop {
            match self.bump() {
                Some('|') => return self.token(TokenKind::Atom, start),
                Some('\\') => {
                    self.bump();
                }
                Some(_) => {}
                None => return self.token(TokenKind::Error, start), // unterminated
            }
        }
    }

    fn lex_hash(&mut self, start: usize) -> Token {
        // Block comment (delimiters may be `#|`..`|#`).
        if let Some(bc) = self.opts.block_comment {
            if self.rest().starts_with(bc.open) {
                return self.lex_block_comment(start, bc.open, bc.close, bc.nestable);
            }
        }

        self.bump(); // consume '#'
        match self.peek() {
            Some(';') if self.opts.datum_comment => {
                self.bump();
                self.token(TokenKind::Prefix(Prefix::Discard), start)
            }
            Some('\\') if self.opts.char_literal => self.lex_char(start),
            Some('(') => {
                self.bump();
                self.token(TokenKind::HashOpen(Delim::Round), start)
            }
            Some('u') if self.rest().starts_with("u8(") => {
                self.bump();
                self.bump();
                self.bump();
                self.token(TokenKind::HashOpen(Delim::Round), start)
            }
            Some('t') if self.opts.booleans => {
                self.bump();
                if self.rest().starts_with("rue") {
                    self.bump();
                    self.bump();
                    self.bump();
                }
                self.token(TokenKind::Bool(true), start)
            }
            Some('f') if self.opts.booleans => {
                self.bump();
                if self.rest().starts_with("alse") {
                    for _ in 0..4 {
                        self.bump();
                    }
                }
                self.token(TokenKind::Bool(false), start)
            }
            Some(c) if is_radix(c) => {
                // Radix/exactness number, e.g. #xFF, #b1010, #e1.0.
                self.consume_atom_body();
                self.token(TokenKind::Atom, start)
            }
            Some(c) if c.is_ascii_digit() && self.opts.datum_labels => self.lex_label(start),
            _ => {
                // Directives (#!fold-case) and any other #tag — capture without
                // choking (ADR-0011). Treated as an atom for now.
                self.consume_atom_body();
                self.token(TokenKind::Atom, start)
            }
        }
    }

    fn lex_block_comment(
        &mut self,
        start: usize,
        open: &str,
        close: &str,
        nestable: bool,
    ) -> Token {
        // consume opener
        for _ in 0..open.chars().count() {
            self.bump();
        }
        let mut depth = 1usize;
        loop {
            if self.rest().is_empty() {
                return self.token(TokenKind::Error, start); // unterminated
            }
            if nestable && self.rest().starts_with(open) {
                for _ in 0..open.chars().count() {
                    self.bump();
                }
                depth += 1;
            } else if self.rest().starts_with(close) {
                for _ in 0..close.chars().count() {
                    self.bump();
                }
                depth -= 1;
                if depth == 0 {
                    return self.token(TokenKind::BlockComment, start);
                }
            } else {
                self.bump();
            }
        }
    }

    fn lex_char(&mut self, start: usize) -> Token {
        self.bump(); // backslash
        if let Some(c) = self.bump() {
            // Named char like #\space / #\newline / #\x41: keep alphanumerics.
            if c.is_alphabetic() {
                while matches!(self.peek(), Some(c) if c.is_alphanumeric()) {
                    self.bump();
                }
            }
        }
        self.token(TokenKind::Char, start)
    }

    fn lex_label(&mut self, start: usize) -> Token {
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.bump();
        }
        match self.peek() {
            Some('=') => {
                self.bump();
                self.token(TokenKind::Label, start)
            }
            Some('#') => {
                self.bump();
                self.token(TokenKind::LabelRef, start)
            }
            // Not actually a label; treat the run as an atom.
            _ => self.token(TokenKind::Atom, start),
        }
    }

    /// Consume symbol-constituent characters up to the next terminator.
    fn consume_atom_body(&mut self) {
        while let Some(c) = self.peek() {
            if self.is_terminator(c) {
                break;
            }
            self.bump();
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        self.next_token()
    }
}

fn is_radix(c: char) -> bool {
    matches!(
        c,
        'e' | 'i' | 'b' | 'o' | 'd' | 'x' | 'E' | 'I' | 'B' | 'O' | 'D' | 'X'
    )
}
