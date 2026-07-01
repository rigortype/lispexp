//! The lexer's output: [`Token`] and [`TokenKind`].

use crate::datum::{Delim, Prefix};
use crate::span::Span;

/// One lexeme. Tokens tile the input — every byte belongs to exactly one token,
/// whitespace and comments included (ADR-0015). Carries only a span; text is
/// recovered by slicing the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The classification of a [`Token`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    /// An opening delimiter.
    Open(Delim),
    /// A closing delimiter.
    Close(Delim),
    /// A `#`-led opening such as `#(` or `#u8(`; the span covers `#`..`(` and
    /// the reader derives the tag from it.
    HashOpen(Delim),
    /// A `#tag` tagged-literal marker (Clojure `#inst`, `#uuid`, `#:ns`, ...);
    /// the reader attaches it to the following datum. Span covers `#`..end of tag.
    HashTag,
    /// A string literal, including its surrounding quotes.
    Str,
    /// A character literal such as `#\a` or `#\space`.
    Char,
    Bool(bool),
    /// A symbol or number; the reader classifies which.
    Atom,
    /// A reader-macro prefix applied to the following datum.
    Prefix(Prefix),
    /// A datum label definition marker `#n=`.
    Label,
    /// A datum label reference `#n#`.
    LabelRef,
    /// A malformed lexeme (e.g. an unterminated string or block comment).
    Error,
}
