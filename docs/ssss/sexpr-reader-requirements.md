# Requirements for an S-expression reader crate (for `cccc` Lisp-family support)

Status: **draft / planning**. Audience: whoever builds the reader crate that
`cccc`'s Lisp-family adapters (`cccc-scheme`, and later Common Lisp, Emacs Lisp,
Clojure) will lower into the complexity IR.

This document collects (a) what `cccc` needs from such a reader, and (b) the
lexical/dialect surface it must cover, based on the R7RS-small target plus an
empirical evaluation of the existing [`s-expr`](https://crates.io/crates/s-expr)
`0.1.1` crate.

---

## 1. Context — what `cccc` does with the parse tree

`cccc` measures Cognitive (SonarSource) and Cyclomatic (McCabe) complexity. Each
language ships a thin **adapter** that lowers a native parse tree into a shared
IR (`cccc_core::ir::Node`: functions, branches, loops, switches, catches, logical
runs, calls). See [ADDING_A_LANGUAGE.md](ADDING_A_LANGUAGE.md).

For a Lisp, "the parse tree" is just the **datum tree** (S-expressions). The
adapter recognises *special forms by their head symbol* (`define`, `if`, `cond`,
`and`, `lambda`, …) and maps them to IR nodes. Crucially, the adapter must also
know **which parts are code and which are data** (a quoted list `'(if x y)` is
data, not an `if`), or it will badly over-count — this was a concrete bug class
in the Ruby work.

So the reader does **not** need evaluation, macro-expansion, a numeric tower, or
exact datum semantics. It needs to produce a faithful, position-annotated,
code-vs-data-aware **structural** tree.

---

## 2. Why not `s-expr 0.1.1` as-is (empirical findings)

`s-expr 0.1.1` is pure-Rust, span-tracking, and handles the S-expression core,
but a probe against real Scheme syntax showed blocking gaps. With
`TokenizerConfig::default().support_bytes(false)`:

| Input | Result | Verdict |
|-------|--------|---------|
| `(define (f x) …)`, `[let ([x 1]) x]` | `Group(Paren/Bracket, …)` with `line:col` spans | ✅ good baseline |
| `; comment` | `Comment(…)` element | ✅ |
| `#t` / `#f` | `Atom("#t")` / `Atom("#f")` | ✅ (only with `support_bytes(false)`) |
| `'foo` | `Atom("'foo")` | ⚠️ quote glued into the atom |
| `'(a b c)` | `Atom("'")` then `Group(…)` | ⚠️ quote is a separate atom |
| `` `(a ,b ,@c) `` | `Atom("` `")`, then `,b`/`,@c` glued into atoms | ⚠️ unquotes glued into atoms |
| `#(1 2 3)` / `#u8(…)` | `Atom("#")`/`Atom("#u8")` then `Group(…)` | ⚠️ `#`-form split into two tokens |
| **`#\a` / `#\space` / `#\(`** | `Atom("#")` then **`TokenizerError(UnprocessedChar('\\'))`** | ❌ **hard parse failure** |
| **`#|…|#`** (block comment) | tokenised as `#|`, `block`, `|#` atoms | ❌ comment body parsed as code |
| **`#;`** (datum comment) | `#` atom, then `;…` eats the rest of the line | ❌ breaks structure |

**Blocking:** character literals (`#\…`) hard-error, and block/datum comments are
mis-tokenised. Both appear in ordinary Scheme. `s-expr 0.1.1` would fail or
mis-measure a large fraction of real files. It could be salvaged with an
offset-preserving preprocessing shim, but once the shim has to understand
strings, char literals, and nested block comments, it *is* most of a reader — so
a purpose-built reader is the cleaner foundation, and it also lets us cover the
other Lisp dialects (below) uniformly.

What `s-expr` got **right**, and we should keep: pure-Rust with ~no deps,
`line:col` spans on every node, `()`/`[]`/`{}` group kinds, `;` comments, and
`support_bytes(false)`-style configurability. Treat this list as a baseline.

---

## 3. Functional requirements (from `cccc`, the consumer)

Priority: **MUST** = needed for a correct first release (Scheme); **SHOULD** =
strongly wanted; **NICE** = future-proofing.

### 3.1 Output structure

- **MUST** parse a whole source string into a sequence of top-level **data**
  (a `Vec<Datum>`), where a `Datum` is one of: list, symbol, number, string,
  char, boolean, keyword, vector/other-literal, or a *prefixed* datum (a reader
  macro applied to an inner datum — see 3.3).
- **MUST** for every list, report its **delimiter kind** (`()` round, `[]`
  square, `{}` curly, and set/`#{}` for Clojure) verbatim — the adapter decides
  per dialect what each delimiter *means* (Scheme `[]` = code list; Emacs Lisp
  `[]` = data vector; Clojure `[]` = vector, `{}` = map). The reader classifies
  shape, not semantics.
- **MUST** expose the **head element** of a list cheaply (first child), and the
  raw **symbol text** of any symbol, so the adapter can dispatch special forms
  and detect recursion by name.
- **MUST** distinguish **symbol vs number vs string vs char vs boolean** at least
  coarsely. Exact numeric value is **not** required; "this atom is a number" is
  enough. Symbol text must be preserved exactly (including `?`/`!`/`->`/`*`/`/`
  and namespace separators).

### 3.2 Source positions

- **MUST** attach, to every datum, at least the **1-based start line** (used for
  `Node::Function { line }` and hotspot reporting). `line:col` (col 0- or
  1-based, documented) as `s-expr` provides is ideal.
- **SHOULD** provide start+end (a span / byte range) so future features (slicing,
  precise diagnostics) are possible.

### 3.3 Reader macros / code-vs-data (correctness-critical)

- **MUST** represent quoting so the consumer can *skip data and descend into
  code*:
  - `quote` / `'x` → a prefixed datum tagged **Quote**; its contents are **data**
    (adapter skips them).
  - `quasiquote` / `` `x `` → tagged **Quasiquote**; contents are data **except**
    nested `unquote` / `unquote-splicing`.
  - `unquote` / `,x`, `unquote-splicing` / `,@x` → tagged **Unquote** /
    **UnquoteSplicing**; contents are **code** again.
- **MUST** attach these prefixes to the correct inner datum, whether the source
  wrote them long-hand (`(quote x)`) or with reader shorthand (`'x`, `` `x ``,
  `,x`, `,@x`). (This is exactly where `s-expr` glued the prefix into the atom.)
- **SHOULD** likewise tag the dialect-specific prefixes (so adapters can treat
  them correctly), e.g. Clojure `#_` (discard → drop the next datum), `#'`
  (var-quote), `@` (deref), `^meta` (metadata); Common Lisp `#'` (function
  quote), `#.` (read-time eval), `#+`/`#-` (reader conditionals). At minimum the
  reader must not *choke* on them and must let the consumer identify/skip them.

### 3.4 Comments & whitespace

- **MUST** skip line comments (`;` for Lisps; note the datum-level ones below)
  and standard whitespace.
- **MUST** handle **block comments** where the dialect has them: Scheme/CL
  `#|…|#` **nestable**.
- **MUST** handle **datum comments** that skip the *next datum*: Scheme `#;`,
  Clojure `#_`. (Getting these wrong silently deletes or duplicates code.)
- **SHOULD** treat Clojure commas as whitespace.

### 3.5 Robustness & integration

- **MUST** be **fault-tolerant**: never panic on malformed input; return a
  *partial* tree plus a list of error messages (mirrors how the PHP/Ruby adapters
  surface `parse_errors` per file). A single bad form must not lose the whole
  file.
- **MUST** be **pure Rust**, no C toolchain / `bindgen` / system libs, so `cccc`
  keeps cross-compiling cleanly to `*-musl` and `*-windows-msvc` (a stated
  project value; the Ruby/Prism adapter is the one heavyweight exception we do
  not want to repeat).
- **MUST** accept arbitrary UTF-8; symbols/strings may contain non-ASCII.
- **SHOULD** be **O(n)** and allocation-frugal — `cccc` walks many files in
  parallel. Zero-copy `&str` slices for atoms (as `s-expr` does) are welcome.
- **SHOULD** offer a **dialect selector** (`Scheme` / `CommonLisp` / `EmacsLisp`
  / `Clojure`) that toggles the lexical rules in §4, since one reader serving all
  four is the goal. A shared core with per-dialect tables is expected.
- **NICE** minimal dependencies (ideally only something like `unicode-xid`).

---

## 4. Lexical surface to cover (per dialect)

The adapter cares about **structure + head symbols + code/data**, so the reader
must at least *tokenise these without error and classify them*; it need not
interpret their values.

### 4.1 Shared S-expression core (all dialects)

- Lists with `(` … `)`.
- Whitespace; `;` line comments to EOL.
- Symbols, integers/floats/ratios, strings `"…"` with escapes (`\"`, `\\`, `\n`,
  …). Note `;`, `#\`, `#|` **inside strings are literal** and must not trigger
  comment/char handling.
- Quote family: `'` `` ` `` `,` `,@`.
- Dotted pairs `(a . b)`.

### 4.2 Scheme (R7RS-small) — the first target

- Brackets `[]` used interchangeably with `()` for **code** (common convention).
- Booleans `#t` `#f` (also `#true` `#false`).
- Characters `#\a`, `#\space`, `#\newline`, `#\tab`, `#\null`, `#\x41` (hex),
  and delimiter chars `#\(`, `#\)`, `#\;`, `#\"`, `#\\`, `#\ ` (space char).
  **These are the tokens `s-expr` hard-fails on.**
- Vectors `#(…)`, bytevectors `#u8(…)` (data).
- Block comments `#|…|#` (**nestable**); datum comments `#;<datum>`.
- Number prefixes `#e #i #b #o #d #x` (e.g. `#xFF`, `#e1.0`), rationals `1/2`.
- Piped symbols `|foo bar|` (symbol containing spaces/specials).
- Datum labels `#0=(…)` / `#0#` (rare; must not crash — may be treated as data).
- Special-form head symbols the Scheme adapter will look for (informative, not
  the reader's concern): `define` `define-values` `define-syntax` `lambda`
  `case-lambda` `let` `let*` `letrec` `letrec*` `let-values` named-`let`
  `if` `when` `unless` `cond` `case` `and` `or` `do` `begin` `guard`
  `parameterize` `delay` `set!` `quasiquote`/`unquote`.

### 4.3 Common Lisp (future)

- `#'fn` (function quote), `#.expr` (read-time eval), `#+feat`/`#-feat` (reader
  conditionals — must at least skip the guarded form), `#\Newline` named chars,
  `#(vector)`, `#B/#O/#X/#nR` radix, `#C(…)` complex, `#P"…"` pathname,
  `#S(…)` structure, `#|…|#` nestable block comments, backquote/`,`/`,@`.
- `|piped symbols|`, keywords `:kw`, package markers `pkg:sym` / `pkg::sym`.
- Symbol case is folded by the readtable — **preserve source text**; case is the
  adapter's problem if ever relevant.

### 4.4 Emacs Lisp (future)

- **`[…]` is a vector literal (data), not a code list** — opposite of Scheme.
  The reader reports `Square`; the elisp adapter treats it as data.
- Character literals use `?`: `?a`, `?\n`, `?\C-x`, `?\M-x`, `?\(`, `?\s`.
- `#'fn` function quote, `` ` `` / `,` / `,@`, `#xNN`/`#o`/`#b` radix.
- Booleans are `t` / `nil` (plain symbols) — no `#t/#f`.
- No block comments; `;` line comments only. Docstrings are ordinary strings.

### 4.5 Clojure (future)

- Delimiters: `()` list, `[]` vector, `{}` map, `#{…}` set — **four kinds**, all
  meaningful; `#()` is an anonymous-function reader macro (**code**).
- Reader macros: `'` quote, `` ` `` syntax-quote, `~` unquote, `~@`
  unquote-splicing, `@` deref, `#'` var-quote, `^meta` / `#^meta` metadata,
  `#_` **discard next datum**, `#"regex"`, `#(…)` fn literal, `%`/`%1`/`%&` args.
- `;` line comments; **commas are whitespace**; no block comments (a `comment`
  macro is used instead — that is code the adapter may choose to ignore).
- Characters `\a` `\newline` `\space` `\tab` `\uNNNN` `\oNNN`.
- Keywords `:kw`, `::auto-ns/kw`, namespaced symbols `ns/sym`.

---

## 5. Suggested data model (sketch, not binding)

```rust
pub enum Dialect { Scheme, CommonLisp, EmacsLisp, Clojure }

pub struct Parsed<'a> {
    pub data: Vec<Datum<'a>>,       // top-level forms, in order
    pub errors: Vec<ParseError>,    // fault-tolerant: partial tree + diagnostics
}

pub struct Datum<'a> {
    pub kind: DatumKind<'a>,
    pub line: u32,                  // 1-based start line (MUST)
    pub span: (u32, u32),           // byte range (SHOULD)
}

pub enum DatumKind<'a> {
    List { delim: Delim, items: Vec<Datum<'a>> },   // () [] {} #{}
    Symbol(&'a str),
    Keyword(&'a str),
    Number(&'a str),                // raw text; value not interpreted
    Str(&'a str),
    Char(&'a str),                  // raw text incl. #\ / ? / \ form
    Bool(bool),
    // reader macro applied to an inner datum:
    Prefixed { prefix: Reader, inner: Box<Datum<'a>> },
    // dialect literals the adapter treats as data:
    HashLiteral { tag: &'a str, inner: Option<Box<Datum<'a>>> }, // #(…), #u8(…), #".."
}

pub enum Delim { Round, Square, Curly, Set }

pub enum Reader {
    Quote, Quasiquote, Unquote, UnquoteSplicing,   // Lisp core
    Discard,                                        // #; (Scheme), #_ (Clojure) — drop inner
    VarQuote, FunctionQuote, Deref, Meta,           // #' , #' , @ , ^  (dialect)
    ReadEval, ReaderConditional(bool),              // #. , #+/#-  (Common Lisp)
}
```

The one hard requirement behind this shape: the consumer can, for any node,
(1) get its start line, (2) if a list, get the delimiter and iterate children and
read the head symbol, and (3) tell whether a subtree is **code or data** and skip
`Discard`/quoted regions while still descending `Unquote`.

---

## 6. Non-goals

- No evaluation, macro-expansion, or symbol resolution.
- No exact numeric-tower parsing (raw token text suffices).
- No full readtable / reader-conditional *semantics* (only enough to skip
  `#+/#-`/`#_`/`#;` correctly).
- No pretty-printing / round-tripping (a printer is out of scope for `cccc`).

---

## 7. Open questions

1. **One crate, four dialects, or a core + per-dialect front-ends?** A shared
   tokenizer core with dialect tables (delimiters, char-literal syntax, comment
   syntax, reader-macro prefixes) seems right; confirm the seam.
2. **Error recovery strategy** — resume at the next top-level form after an error,
   or finer-grained? `cccc` only needs "partial tree + messages", so
   top-level-resync is likely enough.
3. **How much of Clojure `^metadata` / reader tags `#inst`/`#uuid`/custom `#tag`**
   to model now vs. treat as opaque `HashLiteral`.
4. **Bracket policy** must be per-dialect (Scheme code / elisp data / Clojure
   vector) — confirmed it belongs in the adapter, with the reader only reporting
   `Delim`.
