//! sexpp — a pure-Rust reader (lexer + parser) for S-expression syntax across
//! many Lisp dialects.
//!
//! The crate is deliberately reader-only: it does not evaluate, expand macros,
//! or interpret the numeric tower. See `docs/design.md` and `docs/adr/` for the
//! design and the decisions behind it.
//!
//! Two public layers sit over the same [`Options`] (ADR-0015):
//!
//! - [`lex`] / [`Lexer`] — a linear token stream that tiles the input, for
//!   consumers like a parinfer backend that need lexical state, not a tree.
//! - [`parse`] — builds the [`Parsed`] datum tree on top of the lexer.
//!
//! The first implemented dialect is Scheme ([`Options::scheme`]).
#![forbid(unsafe_code)]

mod datum;
mod error;
mod lexer;
mod options;
mod reader;
mod span;
mod token;

pub use datum::{Datum, DatumKind, Delim, Notation, Prefix};
pub use error::ParseError;
pub use lexer::{lex, Lexer};
pub use options::{BlockComment, DelimRole, Dialect, Options};
pub use reader::{parse, read_all, Parsed};
pub use span::Span;
pub use token::{Token, TokenKind};
