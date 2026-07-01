# sexpp — consolidated data model & API design

Status: **design / pre-implementation**. This is the crate's own design sketch (the
*what*). The *why* behind each choice lives in [docs/adr/](./adr/); the domain
vocabulary lives in [CONTEXT.md](../CONTEXT.md). Types below are a binding sketch,
not final signatures.

Reader-only (ADR-0001), Options-driven dialects (ADR-0003), zero-copy borrowed tree
(ADR-0008), top-level error recovery (ADR-0004).

## Core types

```rust
#![forbid(unsafe_code)]   // ADR-0013

/// Result of reading a whole source string. Borrows the source for its lifetime (ADR-0008).
pub struct Parsed<'a> {
    pub lang_line: Option<&'a str>, // e.g. Racket `#lang racket`, captured verbatim, passive (ADR-0012)
    pub data: Vec<Datum<'a>>,       // top-level forms, in source order
    pub errors: Vec<ParseError>,    // fault-tolerant: partial tree + diagnostics (ADR-0004)
}

pub struct Datum<'a> {
    pub kind: DatumKind<'a>,
    pub span: Span,                 // byte range into the source
    pub line: u32,                  // 1-based start line; column derived on demand (ADR-0008)
}

pub struct Span { pub start: u32, pub end: u32 }

pub enum DatumKind<'a> {
    // Shape only; the *meaning* of delim is the consumer's per dialect (ADR-0003/0005/0006).
    // tail: Some(_) => improper/dotted list `(a b . c)` (ADR-0009).
    List { delim: Delim, items: Vec<Datum<'a>>, tail: Option<Box<Datum<'a>>> },

    Symbol(&'a str),   // verbatim slice, incl. |bars| if piped; logical name computed lazily
    Keyword(&'a str),
    Number(&'a str),   // raw text; value never interpreted (numeric tower out of scope)
    Str(&'a str),      // raw slice incl. surrounding quotes and escapes
    Char(&'a str),     // raw slice incl. the #\ / ? / \ lead form
    Bool(bool),

    // Reader macro applied to an inner datum (ADR-0002). notation distinguishes
    // 'x (Shorthand) from (quote x) (Longhand); both unify here so code/data
    // classification is one code path.
    Prefixed { prefix: Prefix, notation: Notation, inner: Box<Datum<'a>> },

    // Any #tag-shaped form; tag is open/unvalidated (ADR-0011). Examples of tag:
    // "" (#(...)), "u8" (#u8(...)), "3a" (ISLisp array), "M"/"S" (LFE), "px" (regex).
    HashLiteral { tag: &'a str, inner: Option<Box<Datum<'a>>> },

    // Datum labels (Scheme/CL/Racket). No graph resolution (ADR-0011).
    Label { id: &'a str, inner: Box<Datum<'a>> }, // #n=<datum>
    LabelRef { id: &'a str },                     // #n#
}

pub enum Delim { Round, Square, Curly, Set }      // ()  []  {}  #{}

/// Renamed from the sketch's `Reader` to avoid clashing with the Reader component
/// (the parsing entry point). See "Naming" below.
pub enum Prefix {
    Quote, Quasiquote, Unquote, UnquoteSplicing,  // core quote family
    Discard,                                        // #; (Scheme) / #_ (Clojure/Phel)
    VarQuote, FunctionQuote, Deref, Meta,           // #'  #'  @  ^
    ReadEval, ReaderConditional(bool),              // #.  #+/#-  (Common Lisp)
    HashFn,                                          // Fennel `#expr`, Clojure/Phel `#(...)`
}

pub enum Notation { Shorthand, Longhand }

pub struct ParseError { pub span: Span, pub line: u32, pub message: String }
```

## API shape

Whole-string, borrowed, one call (ADR-0008). No `io::Read`-streaming variant in v1 —
it fights the zero-copy borrow, and `cccc` reads whole files.

```rust
pub fn parse<'a>(source: &'a str, options: &Options) -> Parsed<'a>;

// Convenience: iterate top-level data lazily over the same core (still borrows source).
pub fn read_all<'a>(source: &'a str, options: &Options) -> impl Iterator<Item = Datum<'a>>;
```

`parse` never panics on malformed input (ADR-0004/0013): it returns a partial tree
plus `errors`, resynchronizing at the next top-level form.

## Options & Dialect presets (ADR-0003, 0005, 0006, 0007)

`Options` is a builder of orthogonal, individually-toggleable syntax settings.
A `Dialect` is just a named preset constructor; presets may layer on one another.

```rust
pub struct Options { /* fields below */ }

impl Options {
    pub fn scheme() -> Self;
    pub fn racket() -> Self;       // layers on scheme(): #: keywords, #hash/#px/#rx/#s/#&, {} as List, #lang
    pub fn common_lisp() -> Self;
    pub fn emacs_lisp() -> Self;
    pub fn clojure() -> Self;
    pub fn phel() -> Self;         // layers on clojure(): #php, extra number suffixes
    pub fn fennel() -> Self;       // :foo => Str (not Keyword); # => HashFn
    pub fn lfe() -> Self;          // #M/#S/#B, #<n>r radix, ;| |; ... no — see below; non-nesting handled per dialect
    pub fn islisp() -> Self;       // [] {} Ordinary; #<n>a arrays
    pub fn autolisp() -> Self;     // block comment `;| ... |;`; quote-only; no char literals
}
```

Orthogonal settings that presets configure (from the dialect survey):

- **Delimiter meaning**, per pair: `[]` and `{}` each independently one of
  `List | Vector | Map | Ordinary` (ADR-0005, ADR-0006). `#{}` set is its own flag.
- **`#(` meaning**: vector literal (`HashLiteral`, e.g. Scheme) vs anonymous-function
  reader macro (`Prefixed{HashFn}`, e.g. Clojure/Phel) — a per-dialect classification,
  because the same `#(` glyph means data in one dialect and code in another.
- **Keyword syntax → output kind**: colon-prefix / colon-postfix / `#:`-prefix, and
  whether the result is a `Keyword` (CL/ISLisp/Clojure/Racket) or a `Str`
  (Fennel `:foo`) (ADR-0006). LFE: colon is an ordinary symbol constituent (no keyword).
- **Block comment**: `(open, close, nestable)` triple (ADR-0007) —
  `("#|","|#",true)` for Scheme/CL/ISLisp/Racket, `("#|","|#",false)` for LFE,
  `(";|","|;",_)` for AutoLISP, none for Emacs Lisp/Fennel.
- **String/char syntax**: R6RS/R7RS vs Elisp vs Clojure `\newline`; presence of char
  literals at all (AutoLISP has none). Raw slices retained regardless.
- **Number vs symbol boundary**: a dialect-configured predicate. Value is never parsed;
  the reader only classifies "is this atom a number in this dialect." When ambiguous,
  default to `Symbol` (a mis-classified number is a harmless leaf; a mis-classified
  symbol could look like data). Handles LFE `123foo`→Symbol, `#b10foo`→error,
  Fennel `.inf`/`.nan`→Number.
- **`#<digits><letter>` token rule**: one shared tokenizer rule for ISLisp `#3a(...)`
  arrays and LFE `#36rHelloWorld` radix numbers (ADR-0006), classified into
  `HashLiteral`/`Number` per dialect.

## Symbols (piped, verbatim)

`Symbol(&'a str)` holds the exact source slice, **including** enclosing `|bars|` for
piped symbols (`|foo bar|`) and any `\` escapes. This preserves exact symbol text
(a requirement) and keeps zero-copy. A helper (`fn name(&self) -> Cow<str>`) computes
the unescaped logical name on demand, allocating only when bars/escapes are present.
Case folding (CL/ISLisp readtable behavior) is **not** applied — the raw slice is kept
and case handling is the consumer's concern.

## Naming resolution

The sketch in `cccc`'s requirements used `enum Reader { Quote, ... }`, which clashes
with **Reader** as the parsing component (the domain term in CONTEXT.md). Resolution:
the reader-macro prefix enum is named **`Prefix`**; **Reader** is reserved for the
parsing component / entry point. Public surface: `Parsed`, `Datum`, `DatumKind`,
`Span`, `Delim`, `Prefix`, `Notation`, `Options`, `ParseError`, and free functions
`parse` / `read_all`.

## Open items deferred to implementation

- AutoLISP block-comment nesting and symbol case-folding: unconfirmed in docs
  (ADR-0007) — verify against a real implementation before locking the preset.
- Exact per-dialect number-vs-symbol predicates: enumerate during each dialect's
  implementation, Scheme first.
