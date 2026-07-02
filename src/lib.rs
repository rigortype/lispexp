//! lispexp — a pure-Rust reader (lexer + parser) for S-expression syntax across
//! many Lisp dialects.
//!
//! The crate is deliberately **reader-only**: it does not evaluate, expand
//! macros, or interpret the numeric tower. It reads source text into data — the
//! shape, positions, and reader-macro structure needed to *statically analyze*
//! Lisp code — and accepts a superset of what any one implementation's reader
//! would, so it is a substrate for tools (linters, indexers, formatters), not a
//! validator (ADR-0030). See `docs/design.md` and `docs/adr/` for the design and
//! the decisions behind it.
//!
//! # Quick start
//!
//! ```
//! use lispexp::{parse, Options};
//!
//! let parsed = parse("(define (square x) (* x x))", &Options::scheme());
//! assert!(parsed.errors.is_empty());
//! assert_eq!(parsed.data[0].head_symbol(), Some("define"));
//! assert_eq!(parsed.data[0].items().unwrap().len(), 3);
//! ```
//!
//! The reader is fault-tolerant — a malformed form loses only itself and
//! recovery resumes at the next top-level form (ADR-0004) — so always inspect
//! [`Parsed::errors`] alongside [`Parsed::data`]. `parsed.errors.is_empty()` is a
//! usable "structurally clean" check.
//!
//! # Choosing a dialect
//!
//! There is one reader; a [`Dialect`] selects a preset of [`Options`] (ADR-0003).
//! lispexp never infers a dialect across files — pick one per input, e.g. by file
//! extension:
//!
//! ```
//! use lispexp::{Dialect, Options};
//!
//! let options = match "core.clj".rsplit('.').next() {
//!     Some("clj" | "cljs" | "cljc" | "edn") => Options::clojure(),
//!     Some("scm" | "ss") => Options::scheme_superset(),
//!     Some("el") => Options::emacs_lisp(),
//!     _ => Options::for_dialect(Dialect::Scheme),
//! };
//! # let _ = options;
//! ```
//!
//! Presets are a starting point: adjust individual fields by assignment
//! afterwards (the settings are orthogonal, ADR-0006).
//!
//! # Two layers
//!
//! Both layers sit over the same [`Options`] (ADR-0015):
//!
//! - [`parse`] — builds the [`Parsed`] datum tree. The common entry point.
//! - [`lex`] / [`Lexer`] — a linear token stream that *tiles* the input (every
//!   byte is covered), for consumers like a parinfer backend that need lexical
//!   state, not a tree. The tree drops comments and whitespace, so a
//!   trivia-sensitive tool reads those here and correlates by byte [`Span`].
//!
//! The lexer's EOF contract: tokens always tile the input, and an unterminated
//! construct at end-of-input is reported as one [`TokenKind::Unterminated`] token
//! carrying the lexical state it was in, rather than an error or a truncated
//! token stream.
//!
//! # Static-analysis utilities
//!
//! Built on the tree, each opt-in and reader-only:
//!
//! - [`walk`] — a pruning visitor that classifies each node as [`Class::Code`]
//!   or [`Class::Data`], so a tool descends into code and skips quoted data;
//!   [`walk_regions`] refines `Data` into prunable [`Region::SealedData`] vs.
//!   porous [`Region::PorousData`] so a `Skip` never drops quasiquoted code
//!   (ADR-0026).
//! - [`annotate`] — tags definition forms (name, arglist, docstring, body,
//!   method dispatch) across dialects, from a bundled per-dialect core plus a
//!   spec harvester that learns a project's own def-macros (ADR-0019/0020,
//!   ADR-0031/0032).
//! - [`indent`] — harvests Emacs Lisp indent specs into a `symbol → IndentSpec`
//!   table (ADR-0022).
//! - [`parse_form_at`] — reads exactly one top-level form at a byte offset, for
//!   incremental re-validation after an edit (ADR-0023).
//! - [`LineIndex`] — maps byte offsets to 1-based (line, byte-column) (ADR-0024).
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
pub use line_index::{LineIndex, Terminator};
pub use options::{
    BlockComment, CharRoles, CharSyntax, DelimRole, Dialect, HashBracket, HashParen, Options,
    ParseDialectError,
};
pub use reader::{parse, parse_form_at, FormAt, Parsed};
pub use span::Span;
pub use token::{Token, TokenKind, UnterminatedKind};
pub use walk::{walk, walk_regions, Class, Region, Walk};
