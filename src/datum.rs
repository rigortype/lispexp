//! The parse tree: [`Datum`] and its kinds.

use crate::span::Span;

/// A single parsed unit of S-expression syntax, annotated with its source span
/// and 1-based start line. Borrows `&'a str` slices from the source (ADR-0008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datum<'a> {
    /// What this datum is.
    pub kind: DatumKind<'a>,
    /// Byte range of this datum in the source.
    pub span: Span,
    /// 1-based start line.
    pub line: u32,
}

/// The shape of a [`Datum`].
#[derive(Debug, Clone, PartialEq, Eq)]
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
        /// Byte span of the `.` separator, present only for an improper list.
        /// A text-based reindenter needs the dot's column to align a tail
        /// continuation under it (the `'(eval . FORM)` font-lock idiom), which
        /// `tail` alone can't give (ADR-0009). `Some` iff `tail` is `Some`; see
        /// [`Datum::dot_span`].
        dot: Option<Span>,
    },
    /// A symbol; verbatim slice, including enclosing `|bars|` if piped.
    ///
    /// The `Symbol`/[`Number`](Self::Number) split is lexical-shape only,
    /// following Scheme-ish rules; the classifier never interprets a value.
    /// Anything ambiguous falls back to `Symbol` — e.g. Common Lisp's `1+` and
    /// `1-` are conventionally functions, not numbers, so a leading digit
    /// alone is not sufficient. Consumers that need a stricter or
    /// dialect-specific numeric grammar reclassify `Symbol`/`Number` text
    /// themselves; lispexp only records the token's shape.
    Symbol(&'a str),
    /// A keyword such as `:foo` or `#:foo`.
    Keyword(&'a str),
    /// A number; raw text, value never interpreted.
    ///
    /// Classification is lexical-shape only (see [`Symbol`](Self::Symbol)):
    /// digits, sign, radix/exactness prefixes (`#x`, `#e`, `#36r...`), decimal
    /// points, ratios, exponent markers, and a trailing complex `i` all
    /// qualify. Clojure's symbolic values (`##Inf`, `##-Inf`, `##NaN`)
    /// classify as `Number` in every dialect, since they are read as numeric
    /// literals regardless of whether the dialect otherwise supports them.
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
        /// The auxiliary datum some prefixes carry, if any: the metadata form
        /// for [`Prefix::Meta`] (`^meta target`: `arg` is the metadata,
        /// `inner` the target) and the feature test for
        /// [`Prefix::FeatureConditional`] (`#+sbcl form`: `arg` is `sbcl`,
        /// `inner` the guarded form). `None` for every other prefix. The
        /// enclosing span covers glyph, `arg`, and `inner`.
        arg: Option<Box<Datum<'a>>>,
    },
    /// Any `#tag`-shaped form; `tag` is captured verbatim and unvalidated
    /// (ADR-0011). E.g. `""` for `#(...)`, `"u8"` for `#u8(...)`. The tag may
    /// contain reader-macro glyphs — `` #`(…) `` (a Scheme `syntax-case`
    /// template) reads as a `HashLiteral` with tag `` "`" ``, and `#,(…)` with
    /// tag `","` — because a `#tag` immediately followed by an opening
    /// delimiter is always this one form. A `#tag` *not* followed by a
    /// delimiter (e.g. `` #`x ``) is instead a [`DatumKind::Symbol`].
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

impl<'a> Datum<'a> {
    /// This datum's symbol text, if it is [`DatumKind::Symbol`].
    pub fn as_symbol(&self) -> Option<&'a str> {
        match self.kind {
            DatumKind::Symbol(s) => Some(s),
            _ => None,
        }
    }

    /// This datum's keyword text, if it is [`DatumKind::Keyword`].
    pub fn as_keyword(&self) -> Option<&'a str> {
        match self.kind {
            DatumKind::Keyword(s) => Some(s),
            _ => None,
        }
    }

    /// This datum's raw number text, if it is [`DatumKind::Number`].
    pub fn as_number(&self) -> Option<&'a str> {
        match self.kind {
            DatumKind::Number(s) => Some(s),
            _ => None,
        }
    }

    /// This datum's raw string text (including the surrounding quotes), if it
    /// is [`DatumKind::Str`].
    pub fn as_str(&self) -> Option<&'a str> {
        match self.kind {
            DatumKind::Str(s) => Some(s),
            _ => None,
        }
    }

    /// This datum's raw character-literal text (including its lead form), if
    /// it is [`DatumKind::Char`].
    pub fn as_char(&self) -> Option<&'a str> {
        match self.kind {
            DatumKind::Char(s) => Some(s),
            _ => None,
        }
    }

    /// This datum's items, if it is a [`DatumKind::List`] of any delimiter
    /// shape.
    pub fn items(&self) -> Option<&[Datum<'a>]> {
        match &self.kind {
            DatumKind::List { items, .. } => Some(items),
            _ => None,
        }
    }

    /// The head symbol of this datum's items, if this is a list whose first
    /// item is a symbol.
    pub fn head_symbol(&self) -> Option<&'a str> {
        self.items()?.first()?.as_symbol()
    }

    /// The byte span of the `.` separator, if this is an improper/dotted list
    /// `(a . b)`. `None` for a proper list or any non-list. Lets a text-based
    /// consumer (e.g. a reindenter) find the dot's column without re-scanning
    /// the source between the last item and the tail.
    pub fn dot_span(&self) -> Option<Span> {
        match &self.kind {
            DatumKind::List { dot, .. } => *dot,
            _ => None,
        }
    }

    /// This datum's source text — sugar for `self.span.text(source)`.
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.span.text(source)
    }
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
    /// `#+feature form` / `#-feature form` — Common Lisp / Emacs Lisp feature
    /// conditional. `include` is the sense: `true` for `#+`, `false` for `#-`.
    /// The feature test is carried in the `Prefixed` datum's `arg`; the guarded
    /// form is `inner`. Gated by [`Options::feature_conditional`].
    ///
    /// [`Options::feature_conditional`]: crate::Options::feature_conditional
    FeatureConditional {
        /// `true` for `#+` (include when the feature holds), `false` for `#-`.
        include: bool,
    },
    /// `#?(...)` / `#?@(...)` — Clojure reader conditional wrapping the next
    /// list. `splicing` is `true` for `#?@`, `false` for `#?`. Gated by
    /// [`Options::reader_conditional`].
    ///
    /// [`Options::reader_conditional`]: crate::Options::reader_conditional
    ReaderConditional {
        /// `true` for `#?@` (splicing), `false` for `#?`.
        splicing: bool,
    },
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
