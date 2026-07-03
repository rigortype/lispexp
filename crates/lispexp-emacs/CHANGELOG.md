# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-03

Initial release: the Emacs-specific integration companion crate for `lispexp` — the home for Emacs knowledge (data and interpreters) kept out of the neutral reader core (ADR-0033). Emacs is the de-facto foundation for Lisp tooling, so this crate bundles what a Lisp formatter / linter / LSP otherwise re-derives. Everything here is **read & interpret, never execute**: it resolves structure and returns verbatim values, and never runs elisp.

### Added

- `indent::bundled_table(Dialect) -> IndentTable`: the standard Emacs indent specs (`Number`/`Defun`) Emacs carries built-in, for `Dialect::EmacsLisp` (empty for other dialects). Harvested from a running Emacs with `cc-mode` and other common packages loaded; the regeneration recipe and provenance are in the module docs. Layer a file's own `harvest_indent_specs` output on top with `IndentTable::merge`.
- `local_vars` module — read Emacs file-local variables into raw `name → value-text` bindings: the leading `-*- … -*-` header cookie (variable form and the bare `-*- mode -*-` shorthand, shebang-aware) and the trailing `Local Variables:` … `End:` block (comment-prefix aware). `file_locals(source) -> FileLocals` with last-wins `get`; an `eval:` entry is surfaced as data (a binding named `eval`), never run.
- `dir_locals` module — a simple evaluator for `.dir-locals.el` (read via `lispexp`'s Emacs Lisp reader). `DirLocals::parse(content)` resolves the mode-keyed alist — `nil` = all modes, both `(MODE . VARS)` and `(MODE VARS…)` forms, and one level of `("subdir" . …)` nesting — into raw `name → value-text` bindings. `for_mode(mode)` returns the applicable top-level vars (nil first, then mode-specific); `for_path(mode, relpath)` additionally applies subdirectory-scoped groups whose directory is an ancestor of the file's path, outer-to-inner (nearest wins), matching on a directory boundary. `eval` entries are surfaced as verbatim data, never run.
