//! The Lexer (Layer 1): source → linear token stream that tiles the input.
//!
//! Robust to incomplete input at character granularity; it never does the
//! top-level resync that is a Reader policy (ADR-0015). Driven entirely by
//! [`Options`] (ADR-0003); covers the Scheme, Clojure, Common Lisp, Emacs Lisp,
//! and Racket surfaces.

use crate::datum::{Delim, Prefix};
use crate::options::{CharSyntax, HashParen, Options};
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

    fn is_whitespace(&self, c: char) -> bool {
        c.is_whitespace() || (self.opts.comma_is_whitespace && c == ',')
    }

    /// Does `c` end an atom / not belong to a symbol?
    fn is_terminator(&self, c: char) -> bool {
        self.is_whitespace(c)
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

        // Whitespace run (commas included where configured).
        if self.is_whitespace(c) {
            while matches!(self.peek(), Some(c) if self.is_whitespace(c)) {
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

        // Character literal with a bare backslash lead (Clojure `\a`).
        if c == '\\' && self.opts.char_syntax == Some(CharSyntax::Backslash) {
            return Some(self.lex_char(start));
        }

        // Character literal with a `?` lead (Emacs Lisp `?a`, `?\C-x`).
        if c == '?' && self.opts.char_syntax == Some(CharSyntax::Question) {
            return Some(self.lex_question_char(start));
        }

        // Prefix glyphs (quote family, deref, meta).
        if let Some(kind) = self.try_prefix() {
            return Some(self.token(kind, start));
        }

        // Otherwise, an atom (symbol, number, or keyword).
        Some(self.lex_atom(start))
    }

    /// Lex an atom, honoring `\`-escapes inside symbols where the dialect allows
    /// them (Common Lisp). Always consumes at least the current character.
    fn lex_atom(&mut self, start: usize) -> Token {
        // First character (may be an escape).
        if self.opts.symbol_escape && self.peek() == Some('\\') {
            self.bump();
            self.bump();
        } else {
            self.bump();
        }
        loop {
            match self.peek() {
                Some('\\') if self.opts.symbol_escape => {
                    self.bump();
                    self.bump();
                }
                Some(c) if !self.is_terminator(c) => {
                    self.bump();
                }
                _ => break,
            }
        }
        self.token(TokenKind::Atom, start)
    }

    fn try_prefix(&mut self) -> Option<TokenKind> {
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
        if Some(c) == self.opts.deref {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::Deref));
        }
        if Some(c) == self.opts.meta {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::Meta));
        }
        None
    }

    fn lex_string(&mut self, start: usize) -> Token {
        self.bump(); // opening quote
        if self.consume_string_body() {
            self.token(TokenKind::Str, start)
        } else {
            self.token(TokenKind::Error, start) // unterminated
        }
    }

    /// Consume a `"..."` body from just after the opening quote. Returns whether
    /// the string was terminated.
    fn consume_string_body(&mut self) -> bool {
        loop {
            match self.bump() {
                Some('"') => return true,
                Some('\\') => {
                    self.bump(); // escaped char
                }
                Some(_) => {}
                None => return false, // unterminated
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

    fn at_line_start(&self) -> bool {
        self.pos == 0 || self.src[..self.pos].ends_with('\n')
    }

    fn lex_hash(&mut self, start: usize) -> Token {
        // Line-leading `#lang <name>` directive (Racket) and `#!` shebang.
        if self.at_line_start() {
            if self.opts.lang_line && self.rest().starts_with("#lang") {
                while !matches!(self.peek(), Some('\n') | None) {
                    self.bump();
                }
                return self.token(TokenKind::LangLine, start);
            }
            if self.opts.shebang_line && self.rest().starts_with("#!") {
                while !matches!(self.peek(), Some('\n') | None) {
                    self.bump();
                }
                return self.token(TokenKind::LineComment, start);
            }
        }

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
            Some('_') if self.opts.discard_underscore => {
                self.bump();
                self.token(TokenKind::Prefix(Prefix::Discard), start)
            }
            Some('\'') if self.opts.hash_apostrophe.is_some() => {
                self.bump();
                let prefix = self.opts.hash_apostrophe.unwrap();
                self.token(TokenKind::Prefix(prefix), start)
            }
            Some('.') if self.opts.read_eval => {
                self.bump();
                self.token(TokenKind::Prefix(Prefix::ReadEval), start)
            }
            Some(c) if self.opts.feature_conditional && (c == '+' || c == '-') => {
                self.bump();
                self.token(
                    TokenKind::Prefix(Prefix::ReaderConditional(c == '+')),
                    start,
                )
            }
            Some('^') if self.opts.meta.is_some() => {
                self.bump();
                self.token(TokenKind::Prefix(Prefix::Meta), start)
            }
            Some('?') if self.opts.reader_conditional => {
                self.bump();
                let splicing = self.peek() == Some('@');
                if splicing {
                    self.bump();
                }
                self.token(
                    TokenKind::Prefix(Prefix::ReaderConditional(splicing)),
                    start,
                )
            }
            Some('"') if self.opts.regex_literal => {
                self.bump(); // opening quote
                if self.consume_string_body() {
                    self.token(TokenKind::Str, start) // regex as a string leaf
                } else {
                    self.token(TokenKind::Error, start)
                }
            }
            Some('{') if self.opts.set_literal => {
                self.bump();
                self.token(TokenKind::Open(Delim::Set), start)
            }
            Some('[') if self.opts.square.is_delimiter() => {
                // Emacs Lisp byte-code objects / Racket `#[...]` vectors — a hash
                // literal over a bracketed group.
                self.bump();
                self.token(TokenKind::HashOpen(Delim::Square), start)
            }
            Some('{') if self.opts.curly.is_delimiter() => {
                // Racket `#{...}` vectors.
                self.bump();
                self.token(TokenKind::HashOpen(Delim::Curly), start)
            }
            Some('(') => match self.opts.hash_paren {
                HashParen::Vector => {
                    self.bump();
                    self.token(TokenKind::HashOpen(Delim::Round), start)
                }
                HashParen::HashFn => {
                    // Leave the `(` for the next token; wrap the list as HashFn.
                    self.token(TokenKind::Prefix(Prefix::HashFn), start)
                }
                HashParen::None => {
                    self.consume_atom_body();
                    self.token(TokenKind::Atom, start)
                }
            },
            Some('\\') if self.opts.char_syntax == Some(CharSyntax::HashBackslash) => {
                self.lex_char(start)
            }
            Some('u')
                if self.opts.hash_paren == HashParen::Vector && self.rest().starts_with("u8(") =>
            {
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
            Some(c) if !self.opts.tagged_literals && is_radix(c) => {
                // Radix/exactness number, e.g. #xFF, #b1010, #e1.0.
                self.consume_atom_body();
                self.token(TokenKind::Atom, start)
            }
            Some(c)
                if !self.opts.tagged_literals && c.is_ascii_digit() && self.opts.datum_labels =>
            {
                self.lex_label(start)
            }
            Some('#') if self.opts.tagged_literals => {
                // Clojure symbolic value: ##Inf, ##-Inf, ##NaN — a self-contained
                // numeric literal, not a tag applied to a following form.
                self.bump(); // second '#'
                self.consume_atom_body();
                self.token(TokenKind::Atom, start)
            }
            Some(_) if self.opts.tagged_literals => {
                // `#inst`, `#uuid`, `#:ns`, custom `#tag` — attach to next datum.
                self.consume_atom_body();
                self.token(TokenKind::HashTag, start)
            }
            _ => {
                // Directives (#!fold-case) and any other #form — capture without
                // choking (ADR-0011). Treated as an atom.
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

    /// Lex a character literal from the current position; the backslash lead has
    /// not been consumed yet (`start` marks the token's beginning, which may be a
    /// preceding `#`).
    fn lex_char(&mut self, start: usize) -> Token {
        self.bump(); // backslash
        if let Some(c) = self.bump() {
            // Named char like #\space / \newline / A: keep alphanumerics.
            if c.is_alphabetic() {
                while matches!(self.peek(), Some(c) if c.is_alphanumeric()) {
                    self.bump();
                }
            }
        }
        self.token(TokenKind::Char, start)
    }

    /// Lex an Emacs Lisp `?`-style character literal: `?a`, `?(`, `?\n`,
    /// `?\C-x`, `?\^I`, `?\x41`. `?` followed by any single char is that char;
    /// after `?\` a modifier/named/hex/octal run may follow.
    fn lex_question_char(&mut self, start: usize) -> Token {
        self.bump(); // '?'
        match self.peek() {
            Some('\\') => {
                self.bump(); // '\'
                if let Some(c) = self.bump() {
                    // Modifier (C-, M-, ^), named, hex, or octal escapes.
                    if c.is_alphanumeric() || c == '-' || c == '^' {
                        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '-' || c == '^')
                        {
                            self.bump();
                        }
                    }
                }
            }
            Some(_) => {
                self.bump(); // a single literal char, e.g. ?( ?; ?)
            }
            None => {}
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
