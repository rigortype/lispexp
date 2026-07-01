//! The parse tree: [`Datum`] and its kinds.

use crate::span::Span;

/// A single parsed unit of S-expression syntax, annotated with its source span
/// and 1-based start line. Borrows `&'a str` slices from the source (ADR-0008).
#[derive(Debug, Clone, PartialEq)]
pub struct Datum<'a> {
    /// What this datum is.
    pub kind: DatumKind<'a>,
    /// Byte range of this datum in the source.
    pub span: Span,
    /// 1-based start line.
    pub line: u32,
}

/// The shape of a [`Datum`].
#[derive(Debug, Clone, PartialEq)]
pub enum DatumKind<'a> {
    /// A list. `tail: Some(_)` marks an improper/dotted list `(a b . c)`
    /// (ADR-0009). The delimiter is shape only; its *meaning* is the consumer's
    /// per dialect.
    List {
        /// The delimiter shape (`()`/`[]`/`{}`/`#{}`).
        delim: Delim,
        /// The list elements, in order.
        items: Vec<Datum<'a>>,
        /// The dotted tail, present only for an improper list `(a . b)`.
        tail: Option<Box<Datum<'a>>>,
    },
    /// A symbol; verbatim slice, including enclosing `|bars|` if piped.
    Symbol(&'a str),
    /// A keyword such as `:foo` or `#:foo`.
    Keyword(&'a str),
    /// A number; raw text, value never interpreted.
    Number(&'a str),
    /// A string; raw slice including the surrounding quotes and escapes.
    Str(&'a str),
    /// A character literal; raw slice including the `#\` / `?` / `\` lead form.
    Char(&'a str),
    /// A boolean.
    Bool(bool),
    /// A reader macro applied to an inner datum (ADR-0002). `notation`
    /// distinguishes `'x` (shorthand) from `(quote x)` (longhand).
    Prefixed {
        /// The reader-macro role.
        prefix: Prefix,
        /// Whether it was written shorthand (`'x`) or longhand (`(quote x)`).
        notation: Notation,
        /// The datum the prefix applies to.
        inner: Box<Datum<'a>>,
    },
    /// Any `#tag`-shaped form; `tag` is captured verbatim and unvalidated
    /// (ADR-0011). E.g. `""` for `#(...)`, `"u8"` for `#u8(...)`.
    HashLiteral {
        /// The text between `#` and the following delimiter (may be empty).
        tag: &'a str,
        /// The datum the tag applies to, if any.
        inner: Option<Box<Datum<'a>>>,
    },
    /// A datum label definition `#n=<datum>` (ADR-0011). No graph resolution.
    Label {
        /// The label id (the digits between `#` and `=`).
        id: &'a str,
        /// The labeled datum.
        inner: Box<Datum<'a>>,
    },
    /// A datum label reference `#n#`.
    LabelRef {
        /// The referenced label id.
        id: &'a str,
    },
}

/// Delimiter shape. The reader records shape; the consumer assigns meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delim {
    /// `(` `)`
    Round,
    /// `[` `]`
    Square,
    /// `{` `}`
    Curly,
    /// `#{` `}` (set)
    Set,
}

/// The role of a reader-macro prefix. The glyph that triggers each is a
/// per-dialect table (ADR-0016).
///
/// `#[non_exhaustive]`: new dialects bring new prefixes without a breaking
/// change; match with a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Prefix {
    /// `'x` / `(quote x)`.
    Quote,
    /// `` `x `` / `(quasiquote x)`.
    Quasiquote,
    /// `,x` / `(unquote x)`.
    Unquote,
    /// `,@x` / `(unquote-splicing x)`.
    UnquoteSplicing,
    /// `#;` (Scheme) / `#_` (Clojure/Phel) — discard the next datum.
    Discard,
    /// `#'x` — Clojure var-quote / Racket syntax.
    VarQuote,
    /// `#'x` — Common Lisp / Emacs Lisp / ISLisp function-quote.
    FunctionQuote,
    /// `@x` — Clojure deref.
    Deref,
    /// `^x` / `#^x` — Clojure metadata.
    Meta,
    /// `#.x` — Common Lisp read-time eval.
    ReadEval,
    /// `#+`/`#-` (CL feature) or `#?` (Clojure); the bool is the include sense.
    ReaderConditional(bool),
    /// Fennel `#expr`, Clojure/Phel `#(...)`, Janet `|(...)`.
    HashFn,
    /// Janet `;x` => `(splice x)`.
    Splice,
    /// Janet `@x` (`@{}` table, `@[]` array, `@""` buffer).
    Mutable,
}

/// Whether a reader-macro form appeared in shorthand or long-hand call form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Notation {
    /// `'x`, `` `x ``, `,x`, `,@x`
    Shorthand,
    /// `(quote x)`, `(quasiquote x)`, ...
    Longhand,
}
