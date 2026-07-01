//! The Lexer (Layer 1): source → linear token stream that tiles the input.
//!
//! Robust to incomplete input at character granularity; it never does the
//! top-level resync that is a Reader policy (ADR-0015). Driven entirely by
//! [`Options`] (ADR-0003); covers the Scheme, Clojure, Common Lisp, Emacs Lisp,
//! and Racket surfaces.

use crate::datum::{Delim, Prefix};
use crate::options::{CharSyntax, HashBracket, HashParen, Options};
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
    /// Create a lexer over `src` configured by `opts`.
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

        // Block comment — checked before the line comment so custom delimiters
        // that share a lead char (AutoLISP `;|...|;` vs `;`) win.
        if let Some(bc) = self.opts.block_comment {
            if self.rest().starts_with(bc.open) {
                return Some(self.lex_block_comment(start, bc.open, bc.close, bc.nestable));
            }
        }

        // Line comment.
        if c == self.opts.line_comment {
            while !matches!(self.peek(), Some('\n') | None) {
                self.bump();
            }
            return Some(self.token(TokenKind::LineComment, start));
        }

        // Backtick long string (Janet).
        if c == '`' && self.opts.long_string_backtick {
            return Some(self.lex_backtick_string(start));
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
        if Some(c) == self.opts.splice {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::Splice));
        }
        if Some(c) == self.opts.mutable {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::Mutable));
        }
        if Some(c) == self.opts.short_fn {
            self.bump();
            return Some(TokenKind::Prefix(Prefix::HashFn));
        }
        None
    }

    /// Lex a Janet backtick long string: a run of N backticks, closed by the
    /// next run of at least N backticks. No escapes.
    fn lex_backtick_string(&mut self, start: usize) -> Token {
        let mut open = 0;
        while self.peek() == Some('`') {
            self.bump();
            open += 1;
        }
        loop {
            match self.peek() {
                None => return self.token(TokenKind::Error, start),
                Some('`') => {
                    let mut close = 0;
                    while self.peek() == Some('`') {
                        self.bump();
                        close += 1;
                    }
                    if close >= open {
                        return self.token(TokenKind::Str, start);
                    }
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    /// Lex a Hy bracket string `#[DELIM[...]DELIM]`. The `#` is already consumed;
    /// the current char is the first `[`.
    fn lex_bracket_string(&mut self, start: usize) -> Token {
        self.bump(); // first '['
        let delim_start = self.pos;
        while !matches!(self.peek(), Some('[') | None) {
            self.bump();
        }
        if self.peek() != Some('[') {
            return self.token(TokenKind::Error, start);
        }
        let closer = format!("]{}]", &self.src[delim_start..self.pos]);
        self.bump(); // second '['
        loop {
            if self.rest().is_empty() {
                return self.token(TokenKind::Error, start);
            }
            if self.rest().starts_with(&closer) {
                for _ in 0..closer.chars().count() {
                    self.bump();
                }
                return self.token(TokenKind::Str, start);
            }
            self.bump();
        }
    }

    /// Lex a Gauche char-set literal `#[...]`. The `#` is already consumed; the
    /// current char is `[`. Consumes up to the matching `]`. A `]` closes the
    /// set (so `#[]` is the empty set); a literal `]` member must be escaped
    /// `\]`; and a complete POSIX class `[:name:]` (optionally negated
    /// `[:^name:]`) holds a `]` that does not close, mirroring Gauche's
    /// `Scm_CharSetRead`. Emitted as an opaque [`TokenKind::Str`] leaf (the
    /// reader does not descend into it).
    fn lex_char_set(&mut self, start: usize) -> Token {
        self.bump(); // '['
        loop {
            match self.peek() {
                None => return self.token(TokenKind::Error, start), // unterminated
                Some(']') => {
                    self.bump();
                    return self.token(TokenKind::Str, start);
                }
                Some('\\') => {
                    self.bump();
                    self.bump(); // escaped char (e.g. `\]`)
                }
                Some('[') => {
                    // A well-formed POSIX class `[:name:]` holds a `]` that does
                    // not close the set. Recognized only as a complete, bounded
                    // token via lookahead; a bare `[` is an ordinary member, so
                    // a malformed `[:` can never consume unbounded input.
                    match posix_class_len(self.rest()) {
                        Some(len) => {
                            for _ in 0..len {
                                self.bump();
                            }
                        }
                        None => {
                            self.bump(); // ordinary `[` member
                        }
                    }
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    /// Lex a Gauche/Mosh regexp literal `#/.../`. The `#` is already consumed;
    /// the current char is `/`. Consumes up to the next unescaped `/`, then an
    /// optional single `i` (case-fold) flag — matching Gauche's `read_regexp`,
    /// which reads exactly one char after the closing `/` and only honors `i`.
    /// Emitted as an opaque [`TokenKind::Str`] leaf. Like Mosh's reader, the
    /// pattern ends at the first unescaped `/` without tracking `[...]` classes.
    fn lex_regex_slash(&mut self, start: usize) -> Token {
        self.bump(); // '/'
        if !self.consume_until_unescaped('/') {
            return self.token(TokenKind::Error, start); // unterminated
        }
        if self.peek() == Some('i') {
            self.bump(); // the sole case-fold flag
        }
        self.token(TokenKind::Str, start)
    }

    fn lex_string(&mut self, start: usize) -> Token {
        let content_start = start + 1;
        self.bump(); // opening quote
        if self.consume_until_unescaped('"') {
            return self.token(TokenKind::Str, start);
        }
        // Unterminated: the scan ran to EOF as one Error token, which would
        // swallow the rest of the file so the reader can never resync (R5).
        // Backtrack to just before the first line-start `(` after the opening
        // quote — overwhelmingly the next top-level form, not string content.
        // A legitimately terminated string (even a multiline one holding a
        // line-start `(`) never reaches here, so this only affects the error
        // path.
        if let Some(cut) = next_line_start_paren(&self.src[content_start..]) {
            self.pos = content_start + cut;
        }
        self.token(TokenKind::Error, start)
    }

    /// Consume characters up to and including the next unescaped `close`, from
    /// just after the opener. `\` escapes the following byte (so `\close` does
    /// not terminate). Returns whether `close` was found before EOF. Shared by
    /// strings (`"`), piped symbols (`|`), and regexp literals (`/`).
    fn consume_until_unescaped(&mut self, close: char) -> bool {
        loop {
            match self.bump() {
                Some(c) if c == close => return true,
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
        if self.consume_until_unescaped('|') {
            self.token(TokenKind::Atom, start)
        } else {
            self.token(TokenKind::Error, start) // unterminated
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
                    TokenKind::Prefix(Prefix::FeatureConditional { include: c == '+' }),
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
                    TokenKind::Prefix(Prefix::ReaderConditional { splicing }),
                    start,
                )
            }
            Some('"') if self.opts.regex_literal => {
                self.bump(); // opening quote
                if self.consume_until_unescaped('"') {
                    self.token(TokenKind::Str, start) // regex as a string leaf
                } else {
                    self.token(TokenKind::Error, start)
                }
            }
            Some('{') if self.opts.set_literal => {
                self.bump();
                self.token(TokenKind::Open(Delim::Set), start)
            }
            Some('[') if self.opts.hash_bracket == HashBracket::CharSet => {
                // Gauche char-set literal `#[...]` — opaque up to the matching
                // `]` (respecting `\]`); may hold raw `(`/`[`/`/` bytes.
                self.lex_char_set(start)
            }
            Some('[') if self.opts.hash_bracket == HashBracket::BracketString => {
                // Hy bracket string `#[[...]]` / `#[DELIM[...]DELIM]`.
                self.lex_bracket_string(start)
            }
            Some('/') if self.opts.regex_slash => {
                // Gauche/Mosh regexp literal `#/.../` with optional trailing
                // flag letters — opaque up to the next unescaped `/`.
                self.lex_regex_slash(start)
            }
            Some('v') if self.opts.bytevector_vu8 && self.rest().starts_with("vu8(") => {
                // R6RS/Mosh bytevector `#vu8(...)`.
                for _ in 0..4 {
                    self.bump();
                }
                self.token(TokenKind::HashOpen(Delim::Round), start)
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

/// In `s` (the content just past an unterminated string's opening quote), find
/// the byte offset of the first line-start `(` — the `(` that is the first
/// non-whitespace character of a line after a `\n`. Returns `None` if no such
/// line exists (then the unterminated string keeps its to-EOF span). Used only
/// on the error path to let the reader resync at the likely next top-level form
/// (R5).
fn next_line_start_paren(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            // Skip leading whitespace on the next line.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'\n' && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'(' {
                return Some(j);
            }
        }
        i += 1;
    }
    None
}

fn is_radix(c: char) -> bool {
    matches!(
        c,
        'e' | 'i' | 'b' | 'o' | 'd' | 'x' | 'E' | 'I' | 'B' | 'O' | 'D' | 'X'
    )
}

/// If `s` begins with a complete Gauche POSIX character-class token `[:name:]`
/// (optionally negated `[:^name:]`), return its byte length. Bounded — Gauche
/// caps the class name at `MAX_CHARSET_NAME_LEN` (11) and requires the closing
/// `:]` — so a malformed `[:` inside a char-set can never consume unbounded
/// input; the caller then treats the `[` as an ordinary member instead.
fn posix_class_len(s: &str) -> Option<usize> {
    let after_open = s.strip_prefix("[:")?;
    let after_caret = after_open.strip_prefix('^').unwrap_or(after_open);
    let name_len = after_caret
        .bytes()
        .take_while(|b| b.is_ascii_alphabetic())
        .count();
    if name_len == 0 || name_len > 11 {
        return None;
    }
    let closed = after_caret[name_len..].strip_prefix(":]")?;
    Some(s.len() - closed.len())
}
