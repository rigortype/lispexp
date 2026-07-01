# lispexp — consolidated data model & API design

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
    HashFn,                                          // Fennel `#expr`, Clojure/Phel `#(...)`, Janet `|(...)`
    Splice,                                         // Janet `;x` => (splice x)
    Mutable,                                        // Janet `@x` (@{} table, @[] array, @"" buffer)
}
// The glyph that triggers each Prefix is a per-dialect table (ADR-0016), e.g. Janet
// maps `~`->Quasiquote, `,`->Unquote, `;`->Splice — not the Scheme/Clojure assignments.

pub enum Notation { Shorthand, Longhand }

pub struct ParseError { pub span: Span, pub line: u32, pub message: String }
```

## Layered API: Lexer (Layer 1) + Reader (Layer 2)  (ADR-0015)

Two public layers over the same `Options`. `cccc` consumes the Reader; a parinfer
backend consumes the Lexer.

- **Lexer** — turns source into a linear `Token` stream that *tiles* the input
  (every byte belongs to exactly one token, whitespace included), robust to
  incomplete/unbalanced input at character granularity. It surfaces strings and
  comments as spans (which the tree omits, ADR-0010) and reports escape state, so a
  consumer can match parens, track indentation, and never miscount a delimiter
  inside `#\(`, `"\)"`, a comment, or a long string.
- **Reader** — builds the `Parsed` datum tree on top of the Lexer.

```rust
pub struct Token { pub kind: TokenKind, pub span: Span }

pub enum TokenKind {
    OpenDelim(Delim), CloseDelim(Delim),   // carries which delimiter, for matching
    Atom,                                   // symbol/number/keyword/bool (coarse; Reader refines)
    Str, Char,
    LineComment, BlockComment,              // surfaced here, unlike the tree
    ReaderMarker,                           // '  `  ~  #  #_  #;  #'  etc. — a prefix/hash lead
    Whitespace,
}

pub fn lex<'a>(source: &'a str, options: &Options) -> impl Iterator<Item = Token>;
```

The Lexer lexes linearly and never performs the top-level resync that is a *parser*
policy (ADR-0004); unclosed strings/comments/parens are reported as such, not recovered.
**Non-goal:** lispexp provides the lexical analysis, not the parinfer paren-inference
algorithm — that stays in the consumer.

## Reader API shape

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
    pub fn guile() -> Self;        // layers on scheme(): #{...}# symbols, #!...!# comments, #!fold-case directives
    pub fn racket() -> Self;       // layers on scheme(): #: keywords, #hash/#px/#rx/#s/#&, {} as List, #lang
    pub fn common_lisp() -> Self;
    pub fn emacs_lisp() -> Self;
    pub fn clojure() -> Self;
    pub fn hy() -> Self;           // Python-style literals; ~/~@ unquote; #[[...]] bracket strings; commas not whitespace
    pub fn phel() -> Self;         // layers on clojure(): #php, extra number suffixes
    pub fn fennel() -> Self;       // :foo => Str (not Keyword); # => HashFn
    pub fn lfe() -> Self;          // #M/#S/#B, #<n>r radix, ;| |; ... no — see below; non-nesting handled per dialect
    pub fn islisp() -> Self;       // [] {} Ordinary; #<n>a arrays
    pub fn autolisp() -> Self;     // block comment `;| ... |;`; quote-only; no char literals
    pub fn janet() -> Self;        // `#` line comments (not `;`); `;`=splice, `@`=mutable; [] tuple, {} struct; backtick long strings
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
  `(";|","|;",_)` for AutoLISP, `("#!","!#",_)` for Guile, none for Emacs Lisp/Fennel.
  (Guile's `#!…!#` and Hy's `#!` shebang share a lead but differ by dialect config.)
- **String/char syntax**: R6RS/R7RS vs Elisp vs Clojure `\newline`; presence of char
  literals at all (AutoLISP has none). Raw slices retained regardless.
- **Bracket / long strings** (ADR-0014): Hy's `#[[...]]` / `#[DELIM[...]DELIM]` and
  Janet's backtick long strings — the same balanced, custom-delimiter mechanism, an
  Options-gated string-lexer feature emitting `Str`. Python-style string prefixes
  (`r"" b"" f""`) are kept as part of the raw `Str` slice.
- **Comma handling**: whitespace (Clojure/Phel), or ordinary/insignificant and usable
  as a numeric digit separator (Hy), or plain insignificant (everyone else).
- **Line-comment introducer** (ADR-0016): `;` for most dialects, but `#` for Janet
  (where `;` is instead the splice reader macro). A per-dialect character, not fixed.
- **Reader-glyph → Prefix role table** (ADR-0016): which punctuation triggers which
  `Prefix` is per-dialect — Janet maps `~`->Quasiquote, `,`->Unquote, `;`->Splice,
  `@`->Mutable, `|`->HashFn; Scheme/Clojure use their own assignments.
- **Symbol delimiters** (ADR-0016): the pairs that delimit a symbol with special
  characters — `|...|` (most), plus `#{...}#` for Guile. A set of pairs, not one form.
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
- Hy opaque-surface cases (ADR-0014): f-string embedded code is kept as an opaque
  `Str` (under-counts embedded complexity — accepted); `foo.bar` dotted access is
  kept as a single `Symbol` rather than expanded to `(. foo bar)`; `#!` shebang is
  skipped as a leading line.

## Definition-form annotator (ADR-0019)

Implemented as the `lispexp::annotate` module — a best-effort utility layer over the
`Datum` tree (not part of the reader core). A `Registry` of `FormSpec`s (from
`emacs_lisp_builtins()` plus `harvest_source()`, which reads a def-macro's own
arglist parameter names and `declare` metadata) drives `annotate_form` /
`annotate_tree`, tagging a definition form's children with `Role`s (Name, Arglist,
Docstring, Declare, Interactive, Body). Validated on `~/.emacs.d/elpa`: 529 specs
harvested from third-party def-macros, 11,754 definition forms annotated across
400 files. Never expands or evaluates macros.

## Implementation status

Implemented dialects: **Scheme** (`Options::scheme`), **Clojure** (`Options::clojure`),
**Common Lisp** (`Options::common_lisp`), **Emacs Lisp** (`Options::emacs_lisp`),
**Racket** (`Options::racket`), **Janet** (`Options::janet`), **Hy** (`Options::hy`),
**AutoLISP** (`Options::autolisp`), **Guile** (`Options::guile`), **Phel**
(`Options::phel`), **Fennel** (`Options::fennel`), **LFE** (`Options::lfe`), and
**ISLisp** (`Options::islisp`).

Scheme, Clojure, Common Lisp, Emacs Lisp, and Racket are each exercised by a
real-world corpus under `tests/corpus/` — chibi-scheme (610 files), clojure/clojure
(142), cl-ppcre (23), lem (627 CL files), magit (49 elisp files), and typed-racket
(1872 files) — all parse with zero errors. Janet, Hy, AutoLISP, Guile, Phel, Fennel,
LFE, and ISLisp are covered by targeted unit tests (`tests/janet_hy_autolisp.rs`,
`tests/guile.rs`, `tests/phel.rs`, `tests/fennel.rs`, `tests/lfe.rs`,
`tests/islisp.rs`) rather than a corpus.

Racket notes: the `#lang <name>` line is captured into `Parsed.lang_line` and not
otherwise acted on (ADR-0012); `#:foo` keywords, `[]`/`{}` as code lists, `#'` syntax,
and `#(`/`#[`/`#{` vectors are handled. A `#lang` that selects a non-s-expression
reader (e.g. Scribble's at-expressions) is out of scope — such files are excluded from
the corpus, not parsed.

Clojure first-cut simplifications (structure is always correct; these concern
retained detail):

- **Metadata** `^meta form` / `#^meta form`: parsed as `Prefixed{Meta, target}` — the
  target is wrapped correctly (one form) but the metadata content is consumed and not
  retained yet.
- **Regex** `#"..."`: kept as a `Str` leaf (the raw slice starts with `#"`), not a
  distinct kind.
- **Symbolic values** `##Inf` / `##-Inf` / `##NaN`: self-contained `Number` leaves.

Common Lisp first-cut simplifications:

- **Feature conditionals** `#+feature form` / `#-feature form`: read as two data; the
  guarded form is wrapped as `Prefixed{ReaderConditional(sense), form}` and the feature
  test is consumed but not retained.
- **Dimensioned/typed literals** `#nA(...)`, `#*1010`, `#C(...)`, `#S(...)`, `#P"..."`,
  `#:sym`: lexed as balanced leaves rather than modeled precisely (no parse error).
- **Custom reader macros** (runtime `set-dispatch-macro-character`): fundamentally out
  of a static reader's reach — such files are excluded from the corpus, not parsed.
