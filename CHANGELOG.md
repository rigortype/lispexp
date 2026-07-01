# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Implements seven lisplens-driven feature requests (ADR-0020..0026): polyglot definition registries, method dispatch signatures, an indent-spec table, structured parse errors with positioned reparse, a line index, an EDN preset, and a code-vs-data walker. A post-implementation audit (adversarial correctness review, API-design review, and dialect fact-checking) is folded in below.

### Added

- `annotate::bundled_registry(Dialect)`: a conservative, high-confidence core `Registry` of definition forms per dialect — the single bundled entry point — plus an optional normalized `Category` hint on `FormSpec` and a `FormSpec::define` builder (stamped `Confidence::Consumer`) so consumers extend or override the bundled core (ADR-0020). `Registry` composes: `iter`/`remove`/`merge` plus `Extend`/`FromIterator`.
- Dispatch/method annotation (ADR-0021): a variable-length `Role::Qualifier`, a separate `Role::DispatchValue` for Clojure `defmethod` (whose arglist must be a square vector, so multi-arity round clauses are not mis-tagged), and a `Role::SpecializedArglist` with `Annotated::specialized_params` splitting each required parameter into a verbatim `(variable, specializer)` pair.
- `annotate::Docstring` placement policy (`None`/`Leading`/`LeadingOrLone`) on `FormSpec`: models CL's "a lone trailing string is a value" rule against elisp/Hy's lone-string docstrings, and lets the `defvar` family (value before doc) annotate correctly.
- `Annotated::form`: the annotated form itself, so a definition's full span is one field away.
- `indent::{IndentTable, IndentSpec, harvest_indent_specs, harvest_indent_specs_into}`: a first-class, **owned** `symbol → IndentSpec` table harvested from Emacs Lisp `(declare (indent …))` and `lisp-indent-function`, independent of the definition registry and mergeable across files (ADR-0022). Vector-literal content is not harvested (data, never executed) and a `nil` spec yields no entry.
- `parse_form_at`: a positioned single-form reparse returning the form, its errors, and the end offset, with spans absolute into the source (ADR-0023).
- `LineIndex`: a public byte-offset ↔ 1-based (line, byte-column) index over a `&str`, with `line_range` (ADR-0024). An overflowing column clamps within its own line.
- `Options::edn()` and `Dialect::Edn`: a data-only preset layered on Clojure with all code-only reader syntax disabled — `#(`, `#'`, `#?`, `#"…"`, `@`, and the quote family `'`/`` ` ``/`~`/`~@`/`^` (ADR-0025).
- `walk` with `Class`/`Walk`: a code-vs-data pruning visitor implementing the quasiquote-depth flip rules and prefix ruling table (ADR-0026). Code-reference prefixes (`#'`, `@`, …) are context-transparent (quasiquoted templates stay data); `#.` is code even under quote; `Walk::Stop` aborts a walk early.
- `Options::scheme_superset()` (`Dialect::SchemeSuperset`): a tolerant `.scm`
  "Scheme superset" preset that reads the reader extensions shared by Gauche,
  Mosh, and Gambit — `#[...]` char-set literals and `#/.../` regexps (as opaque
  `Str` leaves), `#"..."` interpolated strings, `#vu8(...)` bytevectors, and
  both leading-colon `:foo` (Gauche/Guile) and trailing-colon `foo:`
  (Gambit/Gerbil) keywords. `Options::scheme()` stays exact R7RS-small; the
  superset is a strict widening consumers opt into for arbitrary `.scm` files.
  On a full Gauche checkout this drops parse errors from 288 (40 files) to 3
  (1 file). See ADR-0027.
- New `Options` fields backing the above: `char_set_literal`, `regex_slash`,
  `bytevector_vu8`, `keyword_trailing_colon`.
- A Gauche corpus conformance test.

### Changed

- **Breaking:** `ParseError` now carries a structured, `#[non_exhaustive]` `ErrorKind` instead of a free-form `message: String`; the human message is rendered via `Display`. Errors are now comparable and hashable independent of source position, and `MalformedToken` retains the offending text (ADR-0023).
- **Breaking:** `Options`, `Dialect`, `Role`, `Confidence`, `Dispatch`, `Docstring`, `IndentSpec`, `Walk`, `Prefix`, and `ErrorKind`'s payload variants are now `#[non_exhaustive]`, so future syntax toggles, dialects, and variants can be added without a breaking change. Construct `Options` from a preset (e.g. `Options::scheme()`) and adjust fields via `..`, and add a wildcard arm when matching. The per-dialect `*_builtins()` functions are private in favor of `bundled_registry`.
- `#[` dispatch is now a single `Options::hash_bracket: HashBracket`
  (`CharSet` / `BracketString` / `None`) instead of the separate `bracket_string`
  flag, making the competing `#[` meanings mutually exclusive by type.
- Annotator correctness (audit): `defmethod`/`cl-defmethod` docstrings are tagged; `ert-deftest` carries its mandatory `()` arglist; the harvester no longer turns a `&rest args` parameter into a fixed `Arglist` slot; a Name must be a symbol or `(setf foo)` (anonymous Fennel `fn` / decorated Hy `defn` no longer mis-annotate); Clojure `defmulti`/`def`/`defprotocol`/`ns` docstrings are recognized and `deftype` is Kind-only.

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

[Unreleased]: https://github.com/rigortype/lispexp/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/rigortype/lispexp/releases/tag/v0.1.1
[0.1.0]: https://github.com/rigortype/lispexp/releases/tag/v0.1.0
