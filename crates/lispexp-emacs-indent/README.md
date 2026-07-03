# lispexp-emacs-indent

Bundled standard **Emacs indent-spec table** for [`lispexp`](https://crates.io/crates/lispexp) — the data companion to `lispexp::indent` (ADR-0033).

`lispexp::indent` provides the *mechanism* (`IndentSpec`, `IndentTable`, `harvest_indent_specs` for a file's own `declare`/`put` specs). This crate ships the *standard data* Emacs carries built-in (`if` → 2, `defun` → `defun`, `cl-flet` → 1, …), so a formatter can match a file indented by a fully-loaded Emacs without re-harvesting it by hand. The indent *algorithm* (`calculate-lisp-indent`) stays the consumer's — this crate is **data only**.

```rust
use lispexp::Dialect;
use lispexp::indent::harvest_indent_specs;
use lispexp_emacs_indent::bundled_table;

// Start from the bundled standard specs, then layer a file's own on top.
let mut table = bundled_table(Dialect::EmacsLisp);
table.merge(harvest_indent_specs(source));
```

`bundled_table` is populated for `Dialect::EmacsLisp`; other dialects return an empty table (these specs are Emacs-specific).

## Provenance

The table is **harvested from a running Emacs**, not transcribed from a standard. The regeneration recipe (a `dump.el` run under `emacs -Q --batch`, with `cc-mode` and other common packages loaded) is documented in the crate's module docs. Refreshing for a new Emacs release or package set is a change to this crate alone — never a `lispexp` core release.

## License

Apache-2.0.
