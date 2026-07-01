//! The parse tree: [`Datum`] and its kinds.

use crate::span::Span;

/// A single parsed unit of S-expression syntax, annotated with its source span
/// and 1-based start line. Borrows `&'a str` slices from the source (ADR-0008).
#[derive(Debug, Clone, PartialEq)]
pub struct Datum<'a> {
    pub kind: DatumKind<'a>,
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
        delim: Delim,
        items: Vec<Datum<'a>>,
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
        prefix: Prefix,
        notation: Notation,
        inner: Box<Datum<'a>>,
    },
    /// Any `#tag`-shaped form; `tag` is captured verbatim and unvalidated
    /// (ADR-0011). E.g. `""` for `#(...)`, `"u8"` for `#u8(...)`.
    HashLiteral {
        tag: &'a str,
        inner: Option<Box<Datum<'a>>>,
    },
    /// A datum label definition `#n=<datum>` (ADR-0011). No graph resolution.
    Label { id: &'a str, inner: Box<Datum<'a>> },
    /// A datum label reference `#n#`.
    LabelRef { id: &'a str },
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prefix {
    Quote,
    Quasiquote,
    Unquote,
    UnquoteSplicing,
    /// `#;` (Scheme) / `#_` (Clojure/Phel) — discard the next datum.
    Discard,
    VarQuote,
    FunctionQuote,
    Deref,
    Meta,
    ReadEval,
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
