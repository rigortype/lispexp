# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Dialect::Gauche`, `Dialect::Mosh`, and `Dialect::Gambit`: named entry points for the `.scm`-using Scheme implementations, each resolving to the shared `Options::scheme_superset()` reader (ADR-0027) so `Dialect::from_str("gauche")` / `Dialect::ALL` cover them.

### Changed

- Every `Dialect` variant now carries an explanatory doc comment (what the dialect is, and its Scheme/Clojure lineage where relevant) and the variants are grouped by family; `Dialect::Guile` in particular is documented as GNU Guile, the official extension language of the GNU Project, rather than the terse "Guile Scheme".
- `Dialect::Scheme` / `Options::scheme()` now document that the Scheme reader tracks the latest small Scheme standard (currently R7RS-small) and reads earlier RnRS as a subset, rather than pinning a version.
- `Options::emacs_lisp()` is documented as also reading the Emacs Lisp Data format (`lisp-data-mode`, `.eld`); Emacs uses one reader for code and data, so no restricted data-only preset is needed (contrast `Options::edn()`).
- A README "Non-goals" section and ADR-0030 state that lispexp is a faithful reader, not a validator: it reports structural diagnostics (delimiter balance, dangling prefixes, malformed tokens) via `Parsed::errors`, but accepts a per-implementation superset and does not perform dialect-semantic/conformance validation.

## [0.2.0] - 2026-07-02

The first breaking release since 0.1.0. It lands seven lisplens-driven capabilities — polyglot definition registries, method-dispatch annotation, an indent-spec table, structured errors with positioned reparse, a line index, an EDN preset, and a code-vs-data walker — plus a tolerant `.scm` "Scheme superset" preset, then folds in a reader-core refinement pass driven by a three-perspective audit (adversarial correctness, API design, and dialect fact-checking). The parse tree and token stream are reshaped for fidelity, fault-tolerant recovery is hardened (no lost input, a bounded recursion depth), and several per-dialect mis-parses are fixed. Many public enums and structs become `#[non_exhaustive]`, so this is the last release that permits exhaustive matching against them.

### Added

- `annotate::bundled_registry(Dialect)`: a conservative, high-confidence core `Registry` of definition forms per dialect — the single bundled entry point — plus an optional normalized `Category` hint on `FormSpec` and a `FormSpec::define` builder (stamped `Confidence::Consumer`) so consumers extend or override the bundled core (ADR-0020). `Registry` composes via `iter`/`remove`/`merge` plus `Extend`/`FromIterator`.
- Dispatch/method annotation (ADR-0021): a variable-length `Role::Qualifier`, a separate `Role::DispatchValue` for Clojure `defmethod` (whose arglist must be a square vector, so multi-arity round clauses are not mis-tagged), and a `Role::SpecializedArglist` with `Annotated::specialized_params` splitting each required parameter into a verbatim `(variable, specializer)` pair.
- `annotate::Docstring` placement policy (`None`/`Leading`/`LeadingOrLone`) on `FormSpec`: models Common Lisp's "a lone trailing string is a value" rule against Emacs Lisp/Hy's lone-string docstrings, and lets the `defvar` family (value before doc) annotate correctly.
- `Annotated::form`: the annotated form itself, so a definition's full span is one field away.
- `indent::{IndentTable, IndentSpec, harvest_indent_specs, harvest_indent_specs_into}`: a first-class, owned `symbol → IndentSpec` table harvested from Emacs Lisp `(declare (indent …))` and `lisp-indent-function`, independent of the definition registry and mergeable across files (ADR-0022).
- `parse_form_at`: a positioned single-form reparse returning the form, its errors, and the byte offset just past it, with spans absolute into the source (ADR-0023).
- `LineIndex`: a public byte-offset ↔ 1-based (line, byte-column) index over a `&str`, with `line_range` (ADR-0024).
- `Options::edn()` and `Dialect::Edn`: a data-only preset layered on Clojure with all code-only reader syntax disabled — `#(`, `#'`, `#?`, `#"…"`, `@`, and the quote family `'`/`` ` ``/`~`/`~@`/`^` (ADR-0025).
- `walk` with `Class`/`Walk`: a code-vs-data pruning visitor implementing the quasiquote-depth flip rules and prefix ruling table, with `Walk::Stop` to abort a walk early (ADR-0026).
- `Options::scheme_superset()` (`Dialect::SchemeSuperset`): a tolerant `.scm` "Scheme superset" preset reading the reader extensions shared by Gauche, Mosh, and Gambit — `#[...]` char-set literals and `#/.../` regexps (opaque `Str` leaves), `#"..."` interpolated strings, `#vu8(...)` bytevectors, and both leading-colon `:foo` and trailing-colon `foo:` keywords. `Options::scheme()` stays exact R7RS-small; the superset is a strict widening consumers opt into for arbitrary `.scm` files, dropping parse errors on a full Gauche checkout from 288 (40 files) to 3 (1 file). See ADR-0027.
  - Backing `Options` fields: `char_set_literal`, `regex_slash`, `bytevector_vu8`, `keyword_trailing_colon`.
- `Datum` accessors — `as_symbol`/`as_keyword`/`as_number`/`as_str`/`as_char`/`items`/`head_symbol`/`text` — for reaching a datum's contents without matching `DatumKind` by hand.
- `Span::len`/`is_empty`/`contains` and `From<Span> for Range<usize>`; the `u32`-offset / 4 GiB-input contract is now documented on `Span` and on `parse`/`lex`.
- `Dialect::ALL` (a growing, non-exhaustive slice of every known dialect), `Dialect::options()` sugar for `Options::for_dialect`, and a kebab-case `Display`/`FromStr` pair (with `ParseDialectError`) for round-tripping dialect names such as `"common-lisp"`.
- `Options::roles: CharRoles` (ADR-0016): a first-class sub-struct collecting the per-dialect reader-macro prefix glyph table (quote/quasiquote/unquote/splicing-suffix/deref/meta/splice/mutable/short-fn), with `CharRoles::scheme` and `CharRoles::clojure` base tables.
- `Options::hash_curly_symbol` (Guile): `#{foo bar}#` extended symbols lex as one verbatim symbol token, delimited like a piped symbol; mutually exclusive with `set_literal` (ADR-0016).
- `Options::fold_longhand`, `Options::fold_case_insensitive`, and a per-family glyph gate governing when `(quote x)`-style longhand folds into a shorthand `Prefixed` datum: on for the Scheme/Lisp family, off for Clojure/EDN/Janet/Hy/Fennel, case-insensitive for Common Lisp/ISLisp/AutoLISP, and never inside a hash literal's inner (data) list (ADR-0002).
- `Options::dotted_pairs_infix` (Racket): tolerates a second dot in a dotted list as Racket's legitimate infix-dot convention (`(dom . -> . rng)`) instead of flagging it.
- `ErrorKind::DepthLimitExceeded` (a fixed `MAX_DEPTH = 200` recursion cap so `parse` never overflows the stack on pathologically nested input; ADR-0004, ADR-0028) and `ErrorKind::ItemAfterDottedTail` (items after a dotted tail are kept and flagged rather than silently scrambling the list).
- `DatumKind::Prefixed` now carries `arg: Option<Box<Datum>>`, retaining the metadata form for `Meta` (`^meta target`) and the feature test for `FeatureConditional` (`#+sbcl form`) instead of discarding them (ADR-0010).

### Changed

- **Breaking:** `ParseError` now carries a structured, `#[non_exhaustive]` `ErrorKind` instead of a free-form `message: String`; the human message is rendered via `Display`. Errors are comparable and hashable independent of source position, and `MalformedToken` retains the offending text (ADR-0023).
- **Breaking:** `Options`, `Dialect`, `Role`, `Confidence`, `Dispatch`, `Docstring`, `IndentSpec`, `Walk`, `Prefix`, and `ErrorKind`'s payload variants are now `#[non_exhaustive]`. Construct `Options` from a preset (e.g. `Options::scheme()`) and adjust fields by assignment (`opts.square = …`), and add a wildcard arm when matching. The per-dialect `*_builtins()` functions are private in favor of `bundled_registry`.
- **Breaking:** `Prefix::ReaderConditional(bool)` is split into `Prefix::FeatureConditional { include }` (Common Lisp/Emacs Lisp `#+`/`#-`) and `Prefix::ReaderConditional { splicing }` (Clojure `#?`/`#?@`); the bool previously conflated two different constructs behind one shape (ADR-0002, ADR-0026).
- **Breaking:** `DatumKind::Prefixed` gained an `arg` field (see Added); any exhaustive match or construction of `Prefixed` must be updated.
- **Breaking:** `TokenKind::Error` is replaced by `TokenKind::Unterminated(UnterminatedKind)`, distinguishing the seven EOF states the lexer can land in (`Str`, `PipedSymbol`, `BlockComment { depth }`, `LongString`, `BracketString`, `CharSet`, `Regex`) — the state a parinfer-style consumer needs at end of input (ADR-0015).
- **Breaking:** `Lexer<'a>` is now `Lexer<'a, 'o>`, decoupling the source lifetime from the `&Options` borrow (mirroring `Parser<'a, 'o>`), so a temporary `&Options` no longer pins the lexer's lifetime to the call site.
- **Breaking:** nine scattered prefix-glyph `Options` fields (`quote`/`quasiquote`/`unquote`/`splicing_suffix`/`deref`/`meta`/`splice`/`mutable`/`short_fn`) are grouped into `Options::roles: CharRoles` (ADR-0016).
- **Breaking:** `reader::read_all` is removed — it returned a concrete `std::vec::IntoIter` and silently discarded diagnostics, which the fault-tolerant reader is built around surfacing; use `parse(...).data`.
- **Breaking:** `#[` dispatch is a single `Options::hash_bracket: HashBracket` (`CharSet` / `BracketString` / `None`) instead of the separate `bracket_string` flag, making the competing `#[` meanings mutually exclusive by type.
- **Breaking:** longhand folding is now dialect-gated (see Added) — `(quote x)` no longer folds in Clojure/EDN/Janet/Hy/Fennel, and `(QUOTE x)` now folds in Common Lisp (case-insensitively), where it previously did not.
- Fault-tolerant recovery is hardened (ADR-0004): a depth cap bounds recursion so `parse` never overflows the stack; a dangling prefix or discard (`#;`, `'`, …) no longer silently drops the rest of the file; an unterminated string backtracks to the next line-start `(` so following code is recovered instead of lost; and an unclosed list's span now covers the children already parsed inside it.
- Annotator fidelity fixes: `defmethod`/`cl-defmethod` docstrings are tagged; `ert-deftest` carries its mandatory `()` arglist; a `&rest args` parameter is no longer harvested as a fixed `Arglist` slot; a definition name must be a symbol or `(setf foo)`, so anonymous Fennel `fn` / decorated Hy `defn` no longer mis-annotate; and Clojure `defmulti`/`def`/`defprotocol`/`ns` docstrings are recognized while `deftype` is kept Kind-only.
- `#t`/`#f`/`#true`/`#false` now require a terminator (delimiter, whitespace, or EOF) before reading as booleans, so `#thing` and SRFI-4 `#f64(...)` no longer misparse as a boolean plus a stray atom or list.
- An unrecognized `#tag(` now lexes as one `HashOpen` covering the whole `#tag(`, producing a single `HashLiteral { tag, inner }` (e.g. Racket `#hash(...)`, ISLisp `#3a(...)`) instead of a split `Symbol` + `List`, and radix `#Nr...` numbers classify as `Number` (ADR-0011).
- `1+`/`1-` and similar are now `Symbol`s rather than misclassified numbers; number classification is lexical-shape-only with a `Symbol` fallback for ambiguity.
- Emacs Lisp modifier-chain character literals (`?\C-\M-x`) now lex as one character token instead of splitting into a char plus a stray atom.

## [0.1.1] - 2026-07-02

Renames the crate from `sexpp` to `lispexp`. This is a name-only release: the reader, lexer, `Options` presets, and annotator API are identical to 0.1.0. Users of the `sexpp` crate should switch to `lispexp`.

### Changed

- Renamed the crate from `sexpp` to `lispexp`, moving the repository to `rigortype/lispexp`. Depend on `lispexp` and import from `lispexp::…` instead of `sexpp::…`; no other source changes are required.

## [0.1.0] - 2026-07-02

Initial release: a pure-Rust, reader-only lexer and parser for S-expression syntax across 13 Lisp dialects, plus a best-effort Emacs Lisp definition-form annotator. It reads code into a faithful, position-annotated tree; it does not evaluate or expand macros.

### Added

- A zero-copy `Datum` tree reader (`parse`) with fault-tolerant top-level error recovery, dotted-pair support, and longhand-quote folding.
- A layered API (`lex`): a token stream that tiles the input, for consumers such as a parinfer backend that need lexical state rather than a tree.
- `Options` presets for 13 dialects — Scheme, Clojure, Common Lisp, Emacs Lisp, Racket, Janet, Hy, AutoLISP, Guile, Phel, Fennel, LFE, ISLisp — built from orthogonal, individually-toggleable syntax settings.
- `lispexp::annotate`: a definition-form annotator that tags a form's parts (name, arglist, docstring, body) using declared metadata and a spec harvester that reads Emacs Lisp def-macros' own arglist parameter names.
- Continuous parse-conformance corpus tests over real-world code (chibi-scheme, clojure/clojure, cl-ppcre, lem, magit, typed-racket).

[Unreleased]: https://github.com/rigortype/lispexp/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/rigortype/lispexp/releases/tag/v0.2.0
[0.1.1]: https://github.com/rigortype/lispexp/releases/tag/v0.1.1
[0.1.0]: https://github.com/rigortype/lispexp/releases/tag/v0.1.0
