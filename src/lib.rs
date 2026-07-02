//! lispexp — a pure-Rust reader (lexer + parser) for S-expression syntax across
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
//! # Example
//!
//! ```
//! use lispexp::{parse, Options};
//!
//! let parsed = parse("(define (square x) (* x x))", &Options::scheme());
//! assert!(parsed.errors.is_empty());
//! assert_eq!(parsed.data[0].head_symbol(), Some("define"));
//! assert_eq!(parsed.data[0].items().unwrap().len(), 3);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod annotate;
mod datum;
mod error;
pub mod indent;
mod lexer;
mod line_index;
mod options;
mod reader;
mod span;
mod token;
mod walk;

pub use datum::{Datum, DatumKind, Delim, Notation, Prefix};
pub use error::{ErrorKind, ParseError};
pub use lexer::{lex, Lexer};
pub use line_index::LineIndex;
pub use options::{BlockComment, CharSyntax, DelimRole, Dialect, HashBracket, HashParen, Options};
pub use reader::{parse, parse_form_at, read_all, FormAt, Parsed};
pub use span::Span;
pub use token::{Token, TokenKind, UnterminatedKind};
pub use walk::{walk, Class, Walk};
