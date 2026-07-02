//! The lexer's output: [`Token`] and [`TokenKind`].

use crate::datum::{Delim, Prefix};
use crate::span::Span;

/// One lexeme. Tokens tile the input — every byte belongs to exactly one token,
/// whitespace and comments included (ADR-0015). Carries only a span; text is
/// recovered by slicing the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token {
    /// The lexeme's classification.
    pub kind: TokenKind,
    /// The lexeme's byte range in the source.
    pub span: Span,
}

/// The classification of a [`Token`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    /// A run of whitespace (and commas, where they are whitespace).
    Whitespace,
    /// A line comment.
    LineComment,
    /// A block comment.
    BlockComment,
    /// A `#lang <name>` directive line (Racket); captured, then treated as
    /// trivia by the reader (ADR-0012).
    LangLine,
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
    /// A boolean literal (`#t`/`#f`).
    Bool(bool),
    /// A symbol or number; the reader classifies which.
    Atom,
    /// A reader-macro prefix applied to the following datum.
    Prefix(Prefix),
    /// A datum label definition marker `#n=`.
    Label,
    /// A datum label reference `#n#`.
    LabelRef,
    /// A lexeme that ran to end-of-input while a multi-character construct was
    /// still open (e.g. an unterminated string or block comment). Carries the
    /// specific state the lexer was in at EOF, so a parinfer-style state-machine
    /// consumer (ADR-0015) does not need to re-derive it from the token's text.
    Unterminated(UnterminatedKind),
}

/// The lexical state an [`TokenKind::Unterminated`] token was in when input
/// ran out.
///
/// `#[non_exhaustive]` so new dialect-specific unterminated states can be
/// added without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UnterminatedKind {
    /// An unterminated string literal (`"...`).
    Str,
    /// An unterminated piped symbol (`|...`) or Guile extended symbol
    /// (`#{...`) — both are symbol-delimiter pairs (ADR-0016).
    PipedSymbol,
    /// An unterminated block comment. Carries the nesting depth still open at
    /// EOF (1 for a non-nestable comment, since it can only ever be open once).
    BlockComment {
        /// The nesting depth still open when input ran out.
        depth: u32,
    },
    /// An unterminated Janet backtick long-string (`` `...``).
    LongString,
    /// An unterminated Hy bracket string (`#[DELIM[...`), including the case
    /// where the opening delimiter itself was never closed with a second `[`.
    BracketString,
    /// An unterminated Gauche char-set literal (`#[...`).
    CharSet,
    /// An unterminated Gauche/Mosh regexp literal (`#/...` or `` #"..." ``).
    Regex,
}
