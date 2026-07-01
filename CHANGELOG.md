# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

- `Options` and `Dialect` are now `#[non_exhaustive]`, so future syntax toggles
  and dialects can be added without a breaking change. Construct `Options` from
  a preset (e.g. `Options::scheme()`) and adjust fields via `..`, and add a
  wildcard arm when matching on `Dialect`. (Breaking for downstream crates that
  built `Options`/matched `Dialect` exhaustively; warrants a 0.2.0 release.)
- `#[` dispatch is now a single `Options::hash_bracket: HashBracket`
  (`CharSet` / `BracketString` / `None`) instead of the separate `bracket_string`
  flag, making the competing `#[` meanings mutually exclusive by type.

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
