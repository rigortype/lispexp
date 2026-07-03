# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

Initial release: the Emacs-specific integration companion crate for `lispexp` — a home for Emacs knowledge (data and interpreters) kept out of the neutral reader core (ADR-0033). It opens with the bundled standard indent table split out of lisplens's formatter.

### Added

- `indent::bundled_table(Dialect) -> IndentTable`: the standard Emacs indent specs (`Number`/`Defun`) Emacs carries built-in, for `Dialect::EmacsLisp` (empty for other dialects). Harvested from a running Emacs with `cc-mode` and other common packages loaded; the regeneration recipe and provenance are in the module docs. Layer a file's own `harvest_indent_specs` output on top with `IndentTable::merge`.
