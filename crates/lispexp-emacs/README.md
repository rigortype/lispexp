# lispexp-emacs

**Emacs-specific integration** data and interpreters for [`lispexp`](https://crates.io/crates/lispexp) (ADR-0033).

`lispexp` is a dialect-neutral S-expression reader. This companion crate is the home for Emacs-specific knowledge that builds on it — data and interpreters tied to one editor (Emacs), version-sensitive, and kept out of the neutral core. It depends on `lispexp`, never the reverse.

Emacs is not an arbitrary target: Lisp tooling is historically inseparable from Emacs, and the de-facto standard for modern Lisp formatting *is* the indentation engine of Emacs's major modes. This crate is where a Lisp formatter / linter / LSP finds the Emacs knowledge it would otherwise re-derive.

## Scope

- **In scope** — Emacs integration:
  - `indent` — the bundled standard indent-spec table.
  - *planned* `local_vars` — Emacs file-local variables: the leading `-*- … -*-` header cookie and the trailing `Local Variables:` block.
  - *planned* `dir_locals` — a simple evaluator for `.dir-locals.el`.
- **Out of scope, editor-neutral** — file-extension → dialect selection is deliberately the caller's, not Emacs-specific (lispexp ADR-0012, ADR-0034).
- **Out of scope, foreign format** — EditorConfig and the like are not S-expressions; a consumer's rendering-policy concern.

**Read & interpret, never execute.** `.dir-locals.el` / `Local Variables:` blocks may carry `eval` forms; `lispexp` evaluates nothing, so this crate resolves the structural entries and surfaces `eval` forms as data without running them. The consumer keeps the indent *algorithm* (`calculate-lisp-indent`); this crate supplies the *data* it runs on.

## `indent` — bundled Emacs indent table

The standard indent specs Emacs carries built-in (`if` → 2, `defun` → 2, `lambda` → `defun`, …), so a formatter matches a file indented by a fully-loaded Emacs without re-harvesting it by hand.

```rust
use lispexp::Dialect;
use lispexp::indent::harvest_indent_specs;
use lispexp_emacs::indent::bundled_table;

// Start from the bundled standard specs, then layer a file's own on top.
let mut table = bundled_table(Dialect::EmacsLisp);
table.merge(harvest_indent_specs(source));
```

`bundled_table` is populated for `Dialect::EmacsLisp`; other dialects return an empty table (these specs are Emacs-specific).

### Provenance

The table is **harvested from a running Emacs**, not transcribed from a standard. The regeneration recipe (a `dump.el` run under `emacs -Q --batch`, with `cc-mode` and other common packages loaded) is in the `indent` module docs. Refreshing for a new Emacs release or package set is a change to this crate alone — never a `lispexp` core release.

## License

Apache-2.0.
