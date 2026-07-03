# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

Initial release: the bundled standard Emacs indent-spec table split out of lisplens's formatter into a reusable `lispexp` companion crate (ADR-0033).

### Added

- `bundled_table(Dialect) -> IndentTable`: the standard Emacs indent specs (`NUMBER`/`Defun`) Emacs carries built-in, for `Dialect::EmacsLisp` (empty for other dialects). Harvested from a running Emacs with `cc-mode` and other common packages loaded; the regeneration recipe and provenance are in the crate docs. Layer a file's own `harvest_indent_specs` output on top with `IndentTable::merge`.
