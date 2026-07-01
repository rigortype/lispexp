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
`IndentSpec::{ Number(u32), Defun, Function(name), Raw(Datum) }`. The known elisp
grammar (integer, `defun`, a custom indent-function symbol) is captured by type;
`Function(name)` holds the function *name only* — lispexp neither resolves nor runs
it (reader-only) — and anything unexpected falls back to `Raw(Datum)`, staying
faithful.

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
