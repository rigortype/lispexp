# Indent specs are a first-class symbol table, independent of the definition registry

## Context

lisplens owns formatting via a native, spec-driven indenter (lisplens ADR-0011)
and needs per-symbol indent metadata — Emacs's `lisp-indent-function` /
`(declare (indent …))` — to indent faithfully without evaluating code. lispexp's
spec harvester (ADR-0019) already reads `declare (indent …)` for role inference
but does not expose it as consumable output. The naive move is to hang indent
off `FormSpec`, but indent specs are not a property of *definitions*: control
and binding macros like `when`, `dolist`, `cl-loop`, and `with-slots` carry
indent specs too, and none of them is a def-form.

## Decision

**Indent metadata lives in a separate, first-class `symbol → IndentSpec` table,
not on `FormSpec`.** The harvester collects every `(declare (indent …))` (and
the equivalent `lisp-indent-function` signal) across the source into this table,
independent of whether the symbol is a definition. A def-form's FormSpec covers
its role structure (name / arglist / body); indent is an orthogonal axis and
gets its own map — we do not mix two different concerns into one struct.

**`IndentSpec` is a typed enum with a verbatim escape hatch:**
`IndentSpec::{ Number(u32), Defun, Function(name), Raw(text) }`. The known elisp
grammar (integer, `defun`, a custom indent-function symbol) is captured by type;
`Function(name)` holds the function *name only* — lispexp neither resolves nor runs
it (reader-only) — and anything unexpected falls back to `Raw` holding the
spec's verbatim source text, staying faithful. An elisp `nil` spec means "no
special indent" and yields no entry.

**The table is owned, not borrowed.** The table's whole purpose is to be merged
across many files and outlive them all (the indenter keeps one project-wide
table), so `IndentTable` owns its strings — unlike the Datum tree, whose
zero-copy borrow (ADR-0008) is justified by its size. An indent table is dozens
of tiny entries per file; owning costs nothing, and the per-dialect `Registry`
(ADR-0020) already owns for the same reason. `harvest_indent_specs_into`
accumulates into an existing table; `merge` composes tables, later layers
winning.

> **Amended 2026-07-02:** the first implementation borrowed the harvested
> source (`IndentTable<'a>` / `Raw(Datum<'a>)`), which made the
> merge-across-files use case impossible without keeping every source alive.
> This ADR originally did not discuss ownership; the paragraph above records
> the corrected decision.

**Indent harvesting is Emacs-Lisp-specific for now.** Like ADR-0019's harvester,
the source of indent specs is elisp's `declare`/`lisp-indent-function`. Other
dialects express indentation differently (Clojure's `:style/indent` lives in
metadata, not `declare`; CL has no standard spec), so the `IndentSpec` *type* is
general but is populated only from elisp today. Other dialects' indent specs may
later be supplied by the consumer.

## Consequences

- lisplens's indenter can indent non-definition forms (`when`, `dolist`, …)
  correctly, which a FormSpec-only design could not serve.
- Exposing indent is cheap — the harvester already reads it; this ADR only makes
  it consumable output.
- Consistent with reader-only scope (ADR-0001): lispexp reads declared metadata and
  hands it over verbatim-typed, never executing an indent function.
