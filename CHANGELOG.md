# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-02

Initial release: a pure-Rust, reader-only lexer and parser for S-expression syntax across 13 Lisp dialects, plus a best-effort Emacs Lisp definition-form annotator. It reads code into a faithful, position-annotated tree; it does not evaluate or expand macros.

### Added

- A zero-copy `Datum` tree reader (`parse`) with fault-tolerant top-level error recovery, dotted-pair support, and longhand-quote folding.
- A layered API (`lex`): a token stream that tiles the input, for consumers such as a parinfer backend that need lexical state rather than a tree.
- `Options` presets for 13 dialects — Scheme, Clojure, Common Lisp, Emacs Lisp, Racket, Janet, Hy, AutoLISP, Guile, Phel, Fennel, LFE, ISLisp — built from orthogonal, individually-toggleable syntax settings.
- `lispexp::annotate`: a definition-form annotator that tags a form's parts (name, arglist, docstring, body) using declared metadata and a spec harvester that reads Emacs Lisp def-macros' own arglist parameter names.
- Continuous parse-conformance corpus tests over real-world code (chibi-scheme, clojure/clojure, cl-ppcre, lem, magit, typed-racket).

[Unreleased]: https://github.com/rigortype/lispexp/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rigortype/lispexp/releases/tag/v0.1.0
