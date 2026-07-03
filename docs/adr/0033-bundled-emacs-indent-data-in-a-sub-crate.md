# Bundled Emacs indent-spec *data* ships as a companion sub-crate, not the reader core

## Context

`lispexp::indent` owns the *mechanism* for editor-style indentation metadata:
the `IndentSpec` enum, the `IndentTable` symbol map, and
`harvest_indent_specs`, which reads a *file's own* `(declare (indent …))` /
`(put 'sym 'lisp-indent-function …)` / `function-put` / `lisp-indent-hook`
forms (ADR-0022). What it does **not** ship is the *standard data* — the large
table of indent specs Emacs itself carries built-in (`if` → 2, `defun` →
`defun`, `cl-flet` → 1, …).

Every consumer that wants Emacs-faithful indentation must therefore reproduce
that data by hand. lisplens did exactly this: a 342-entry
`(symbol → IndentSpec)` table (`NUMBER_SPECS` / `DEFUN_SPECS`), harvested once
from a *running* Emacs — and it had to be re-harvested mid-project with
`(require 'cc-mode)` loaded when `c-lang-defconst` turned out missing (php-mode
uses it heavily). The lisplens retrospective (2026-07-03) ranks *"ship a bundled
default indent-spec table"* its highest-value delegation: it would delete that
table outright and spare the next consumer the same harvest. The clean split it
proposes: **lispexp owns the data, the consumer owns the indent algorithm**
(`calculate-lisp-indent`, which is rendering — out of reader-only scope,
ADR-0001).

The open question this ADR settles is *where* the data lives: the reader core
crate, or a separate home.

## Decision

**The bundled Emacs indent-spec data lives in a companion crate,
`lispexp-emacs-indent`, a workspace member in this repository — not in the
`lispexp` core crate.** The core keeps the mechanism (`IndentSpec`,
`IndentTable`, `harvest_indent_specs`); the companion crate depends on `lispexp`
and exposes:

```rust
pub fn bundled_table(dialect: Dialect) -> IndentTable
```

returning the built-in table for `Dialect::EmacsLisp` and an empty table for
every other dialect (these specs are Emacs-specific; no other target dialect has
an equivalent standard set — ADR-0031). A consumer layers a file's harvested
specs on top with `IndentTable::merge`, exactly as before.

**Why a sub-crate rather than `indent::bundled_table` in core:**

- **The data is *editor-and-version-specific*, the reader core is
  dialect-neutral.** `lispexp` reads S-expression *syntax* across ~15 Lisps and
  evaluates nothing. This table is one editor's (Emacs's) indentation
  configuration, valid for a *pinned Emacs version with specific packages
  loaded*. Baking it into the neutral reader would put version/provenance churn
  and ~350 entries of Emacs data on every `lispexp` user, including the many who
  never touch Emacs indentation.
- **Provenance needs a documented, reproducible home.** The data is *harvested
  from a running Emacs*, not transcribed from a standard. It must record which
  Emacs version and which loaded packages (`cc-mode`, …) produced it, and a
  recipe to regenerate it. A dedicated crate is the natural place for that
  contract; the core crate has no such generated-data story.
- **It composes without coupling.** The companion depends on `lispexp` (for the
  types), never the reverse, so `lispexp`'s own build, tests, and `cargo
  publish` are unaffected, and the core stays free of a data-regeneration
  dependency.

**Why not keep it in lisplens:** indent specs are *metadata about symbols* —
reader-adjacent "data about Lisp," the same shape as the def-form registry
`annotate::bundled_registry` already bundles — so leaving it in one consumer
just guarantees the next consumer re-harvests Emacs by hand, the exact
duplication this repo exists to prevent.

**Relation to `bundled_registry` (which *is* in core):** the def-form registry
is smaller, hand-curated, and closer to language-standard def-forms; the Emacs
indent table is bulk editor data with a regeneration recipe. The distinction
this ADR draws is deliberate — *curated, standard-ish symbol metadata* may live
in core; *bulk, editor-version-specific, harvested data* goes to a companion
crate. `bundled_registry` is not being moved; this sets the pattern for new bulk
data going forward.

**Boundary, restated:**

| Layer | Owner |
|---|---|
| `IndentSpec` / `IndentTable` / `harvest_indent_specs` (mechanism) | `lispexp` core |
| Bundled Emacs standard indent data + provenance/recipe | `lispexp-emacs-indent` |
| `calculate-lisp-indent` indent *algorithm* (rendering) | the consumer (lisplens) |

## Consequences

- lisplens deletes its 342-entry table and depends on `lispexp-emacs-indent`;
  future consumers get the same data for free. The reader core stays lean and
  dialect-neutral.
- The companion crate carries a provenance note: the Emacs version and the
  packages loaded at harvest time, plus the dump recipe, so the table can be
  regenerated deterministically and audited. Refreshing for a new Emacs release
  is a companion-crate change, not a core release.
- Publishing is decoupled: `lispexp-emacs-indent` versions and publishes on its
  own cadence (it depends on a published `lispexp`), so a data refresh never
  forces a core version bump. Wiring its own tag/publish flow is follow-up work;
  in-repo path/git use unblocks lisplens immediately.
- This repository becomes a Cargo workspace (root `lispexp` package +
  `crates/lispexp-emacs-indent`). The single-context domain layout (one
  `CONTEXT.md` + `docs/adr/`) is unchanged.
- If `lispexp` ever grows a formatting layer, the indent *algorithm* would be a
  further crate (e.g. `lispexp-fmt`), with this data crate as its natural first
  half. Out of scope here (ADR-0001 keeps rendering out of core).
