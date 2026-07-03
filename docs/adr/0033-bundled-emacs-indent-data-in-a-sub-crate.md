# Emacs-specific integration lives in a companion crate (`lispexp-emacs`), not the reader core — starting with the bundled indent table

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

The open question this ADR settles is *where* the data lives — the reader core
crate, or a separate home — and, once separate, at *what unit*: a crate for this
one table, or a broader home for Emacs-specific knowledge generally (the indent
table is only the first of several: a major-mode registry, a `.dir-locals.el`
interpreter, …).

## Decision

**Emacs-specific integration lives in one companion crate, `lispexp-emacs`, a
workspace member in this repository — not in the `lispexp` core crate.** The
core keeps the mechanism (`IndentSpec`, `IndentTable`, `harvest_indent_specs`);
`lispexp-emacs` depends on `lispexp` and holds the Emacs-specific *data and
interpreters*, one module per concern. Its first tenant is the bundled indent
table:

```rust
pub fn lispexp_emacs::indent::bundled_table(dialect: Dialect) -> IndentTable
```

returning the built-in table for `Dialect::EmacsLisp` and an empty table for
every other dialect (these specs are Emacs-specific; no other target dialect has
an equivalent standard set — ADR-0031). A consumer layers a file's harvested
specs on top with `IndentTable::merge`.

**The crate's unit is "Emacs-specific integration," not "Emacs indent."** These
concerns cohere — all make a tool match Emacs's view of Lisp, all are
Emacs-version-sensitive, all depend only on `lispexp` — so one crate (modules,
optionally feature-gated) beats both a single-table crate (too narrow; the next
Emacs concern needs a new crate) and per-concern crates (proliferation,
versioning/discoverability overhead).

**Why Emacs is the foundation, not an arbitrary choice.** Lisp tooling is
historically inseparable from Emacs (GNU Emacs and the earlier Emacsen), and —
concretely — the *de-facto standard for modern Lisp formatting is the
indentation engine of Emacs's major modes*. A neutral crate that ignored Emacs
would not be neutral; it would be less useful. So `lispexp-emacs` is
deliberately the home for the Emacs knowledge every Lisp formatter / linter /
LSP otherwise re-derives. Planned modules:

- `indent` (this change) — the bundled standard indent-spec table.
- `local_vars` — readers for Emacs **file-local variables**: the leading
  `-*- mode: …; lexical-binding: t; … -*-` header cookie and the trailing
  `Local Variables:` … `End:` block, surfaced as a key/value map.
- `dir_locals` — a *simple evaluator* for `.dir-locals.el`: read the elisp alist
  (via `lispexp`'s own reader) and resolve its `(mode . ((var . value) …))` /
  per-directory / `nil`-mode structure into applicable variables.

**Safety boundary — read & interpret, never execute.** `.dir-locals.el` and a
`Local Variables:` block can, in Emacs, carry `eval` forms that run arbitrary
elisp. `lispexp` evaluates nothing (ADR-0001), so this crate resolves only the
*structural* entries (mode/directory → variable/value) and **surfaces `eval`
entries as data without executing them** (a limited opt-in evaluation could be
layered later, but never arbitrary elisp). This keeps the reader-only spirit
while still covering the overwhelming common case — `indent-tabs-mode`,
`lexical-binding`, mode-scoped variables, and the like.

**Two axes bound what belongs in `lispexp-emacs` — and what does not:**

- **Editor-*neutral* concerns stay out.** File-extension → dialect selection is
  not Emacs-specific and is *deliberately the caller's*, not the core's
  (ADR-0012); it must not ride in an Emacs crate. If its duplication ever earns
  a home, that is a separate editor-neutral data crate, not this one.
- **Foreign, non-S-expression formats stay out entirely.** EditorConfig and the
  like are not S-expressions; parsing them pulls a foreign format and deps into
  the `lispexp` family, and mapping their properties to Lisp formatting is
  rendering *policy*. That belongs in the consumer (or its own unrelated crate),
  never in `lispexp` or `lispexp-emacs` (ADR-0001, ADR-0013).

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
| Bundled Emacs standard indent data + provenance/recipe (`indent` module) | `lispexp-emacs` |
| `calculate-lisp-indent` indent *algorithm* (rendering) | the consumer (lisplens) |

## Consequences

- lisplens deletes its 342-entry table and depends on `lispexp-emacs`
  (`indent::bundled_table`); future consumers get the same data for free. The
  reader core stays lean and dialect-neutral.
- The companion crate carries a provenance note: the Emacs version and the
  packages loaded at harvest time, plus the dump recipe, so the table can be
  regenerated deterministically and audited. Refreshing for a new Emacs release
  is a companion-crate change, not a core release.
- Publishing is decoupled: `lispexp-emacs` versions and publishes on its own
  cadence (it depends on a published `lispexp`), so a data refresh — or a new
  Emacs module — never forces a core version bump. Wiring its own tag/publish
  flow is follow-up work; in-repo path/git use unblocks lisplens immediately.
- This repository becomes a Cargo workspace (root `lispexp` package +
  `crates/lispexp-emacs`). The single-context domain layout (one `CONTEXT.md` +
  `docs/adr/`) is unchanged.
- If `lispexp` ever grows a formatting layer, the indent *algorithm* would be a
  further crate (e.g. `lispexp-fmt`), with this data crate as its natural first
  half. Out of scope here (ADR-0001 keeps rendering out of core).
