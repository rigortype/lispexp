//! Parse diagnostics (ADR-0023).

use std::fmt;

use crate::datum::{Delim, Prefix};
use crate::span::Span;

/// A structured classification of a parse diagnostic (ADR-0023).
///
/// `#[non_exhaustive]` so new kinds can be added without a breaking change.
/// Variants may carry **non-positional** payload that sharpens identity and
/// diagnostics (e.g. the expected/found delimiter) but never a `Span`-derived
/// value, so a kind stays stable across the position shifts an edit causes. The
/// human-readable message is produced by [`fmt::Display`], not stored.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A closing delimiter with no matching open (e.g. `)` at top level).
    #[non_exhaustive]
    UnexpectedDelimiter {
        /// The delimiter shape that was found.
        found: Delim,
    },
    /// A closing delimiter that does not match the list it closes.
    #[non_exhaustive]
    MismatchedDelimiter {
        /// The delimiter the open list expected.
        expected: Delim,
        /// The delimiter actually found.
        found: Delim,
    },
    /// A list opened but never closed before end of input.
    #[non_exhaustive]
    UnclosedList {
        /// The shape of the unclosed opening delimiter.
        open: Delim,
    },
    /// A token the lexer could not form. Carries the offending text (owned,
    /// non-positional — two different malformed tokens stay distinguishable
    /// in an error-set diff).
    #[non_exhaustive]
    MalformedToken {
        /// The token's verbatim text.
        text: Box<str>,
    },
    /// A reader-macro prefix (`'`, `` ` ``, `,`, `^`, `#+`, …) with no datum
    /// following it.
    #[non_exhaustive]
    DanglingPrefix {
        /// The prefix left dangling.
        prefix: Prefix,
    },
    /// A `#tag` tagged literal with no datum following it.
    DanglingTag,
    /// A `#n=` datum label with no datum following it.
    DanglingLabel,
    /// A `.` inside a list with no tail datum after it.
    DanglingDot,
    /// One or more items appeared after a dotted tail (`(a . b c)`), or a second
    /// dot (`(a . b . c)`). The reader keeps accumulating the stray items into
    /// the list so nothing is lost, but reports this once per offending item so
    /// round-trip consumers know the order was disturbed.
    ItemAfterDottedTail,
    /// Nesting exceeded the reader's depth cap (ADR-0004). The too-deep subtree
    /// is skipped so the reader never overflows the stack; reported once.
    DepthLimitExceeded,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::UnexpectedDelimiter { found } => {
                write!(f, "unexpected closing delimiter `{}`", close_glyph(*found))
            }
            ErrorKind::MismatchedDelimiter { expected, found } => write!(
                f,
                "mismatched closing delimiter: expected `{}`, found `{}`",
                close_glyph(*expected),
                close_glyph(*found)
            ),
            ErrorKind::UnclosedList { open } => {
                write!(f, "unclosed list opened with `{}`", open_glyph(*open))
            }
            ErrorKind::MalformedToken { text } => write!(f, "malformed token `{text}`"),
            ErrorKind::DanglingPrefix { prefix } => {
                write!(f, "{prefix:?} prefix with no following datum")
            }
            ErrorKind::DanglingTag => write!(f, "tagged literal with no following datum"),
            ErrorKind::DanglingLabel => write!(f, "datum label with no following datum"),
            ErrorKind::DanglingDot => write!(f, "dotted list with no tail datum"),
            ErrorKind::ItemAfterDottedTail => {
                write!(f, "item after dotted tail")
            }
            ErrorKind::DepthLimitExceeded => {
                write!(f, "nesting too deep; stopped descending")
            }
        }
    }
}

/// The opening glyph for a delimiter shape (for messages).
fn open_glyph(delim: Delim) -> &'static str {
    match delim {
        Delim::Round => "(",
        Delim::Square => "[",
        Delim::Curly => "{",
        Delim::Set => "#{",
    }
}

/// The closing glyph for a delimiter shape (for messages).
fn close_glyph(delim: Delim) -> &'static str {
    match delim {
        Delim::Round => ")",
        Delim::Square => "]",
        Delim::Curly | Delim::Set => "}",
    }
}

/// A non-fatal parse diagnostic. The reader is fault-tolerant (ADR-0004): it
/// returns a partial tree plus a list of these, resynchronizing at the next
/// top-level form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParseError {
    /// Byte range the diagnostic points at.
    pub span: Span,
    /// 1-based line.
    pub line: u32,
    /// The structured classification of the diagnostic (ADR-0023). The
    /// human-readable message is `kind`'s [`fmt::Display`].
    pub kind: ErrorKind,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.kind)
    }
}

impl std::error::Error for ParseError {}
