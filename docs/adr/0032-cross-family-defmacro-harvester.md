# Cross-family harvesting: the def-macro arglist heuristic ports to every macro-defining Lisp; `syntax-rules` and procedural macros do not

## Context

ADR-0019 built the spec harvester Emacs-Lisp-specific, but noted its mechanism
is general. ADR-0031 answered the question for the Scheme family (no
Edebug/`declare` analog; the harvestable signal is the `syntax-rules` *pattern*).
This ADR settles the rest of the dialect matrix: which families carry a
harvestable signal for their *own* project-local def-macros, and which single
harvester covers them.

Surveying the families for machine-readable, statically-visible metadata about a
def-macro's argument structure:

- **No family but Emacs Lisp has a dedicated arg-structure spec** (elisp's
  Edebug `(declare (debug (&define …)))`). Nothing equivalent exists in Common
  Lisp, Clojure, Fennel, Janet, Hy, LFE, ISLisp, or the Scheme standards.
- **Indentation/structure metadata is mostly editor-external.** Common Lisp's
  lives in Emacs `cl-indent.el` / SLIME; Scheme's in `scheme.el` / DrRacket.
  **Clojure is the exception**: `:arglists`/`:doc`/`:style/indent` are in-source
  metadata — the closest analog to elisp — but `:style/indent` is a CIDER
  convention with low adoption, not a language standard, and reading it well
  needs metadata-map parsing beyond this heuristic. **Fennel** and **Janet** do
  store arglist/docstring metadata, but at *runtime* (`fnl/arglist`,
  Janet's `:doc` on the binding), not as a statically-declared spec.
- **The one portable, statically-visible signal is the def-macro's own arglist
  parameter names** — exactly ADR-0019's highest-yield elisp heuristic. Every
  one of these families defines macros as `(HEAD name ARGLIST body…)` where
  `ARGLIST` literally names the roles (`name`, `args`, `body`, `docstring`). The
  differences between families are small and mechanical:
  - **Arglist delimiter:** `()` list (elisp, Common Lisp, ISLisp, LFE) vs. `[]`
    vector (Clojure, Fennel, Janet, Hy). lispexp already reads both as
    `DatumKind::List` of a delimiter shape, so no special handling is needed.
  - **Rest/optional markers:** all `&`-prefixed (`&rest`/`&body`/`&optional` in
    CL/elisp, `&`/`&opt` in Clojure/Janet), with ISLisp's `:rest` the lone
    outlier. A uniform rule — `&rest`/`&body`/`&` open the body, any other
    `&…`/`:rest` is a non-role marker to skip — covers them all.
  - **Macro-defining head:** `defmacro` for most, `cl-defmacro` (elisp),
    `defmacro-` (Janet), and `macro` (Fennel).
  - **Docstring:** elisp/Hy allow a lone trailing string; CL/Clojure/Janet/
    Fennel/LFE treat it positionally; ISLisp has none. This matters only when a
    def-macro's arglist actually *names* a docstring parameter (rare outside
    elisp), so it is a per-dialect policy applied at that point.
- **`syntax-rules` and procedural macros stay out of reach.** The Scheme family
  (`syntax-rules` patterns, ADR-0031) and any procedural transformer
  (`syntax-case`, `er/rsc/sc-macro-transformer`, Gauche legacy `define-macro`)
  expose no arglist, so this harvester yields nothing for them — consistent with
  elisp macros that lack a `&define` spec.

## Decision

**Generalize the harvester with a small per-dialect `HarvestProfile`, keeping
the existing arglist-name mechanism.** Add `harvest_source_for(source, dialect,
reg)`; `harvest_source` becomes its `Dialect::EmacsLisp` shorthand
(back-compatible). A `HarvestProfile` carries only what differs between families:
the macro-defining heads, the docstring policy (which `Docstring` variant a
harvested spec gets when its arglist names a doc parameter), and whether to apply
the elisp `declare` refinement. Marker handling is unified with the `&`-prefix
rule above, and arglist-delimiter differences need no code because `[]` and `()`
are both `DatumKind::List`.

`harvest_profile` returns `None` for the Scheme family (Scheme, Guile, Gauche,
Mosh, Gambit, superset, Racket — `syntax-rules`, ADR-0031) and for EDN/AutoLISP
(no user macros), so `harvest_source_for` returns `0` there rather than
mis-harvesting.

Harvested specs keep their provenance: `Confidence::Inferred` from arglist names,
`Confidence::Declared` only where an elisp `declare` corroborates. The annotator
mechanism (`annotate_form`) was already dialect-agnostic and is unchanged.

## Consequences

- A consumer can now harvest project-local def-macros for Common Lisp, Clojure,
  Phel, Fennel, Janet, Hy, LFE, and ISLisp with the same call shape as elisp —
  the annotator "beyond Emacs Lisp" goal, delivered for every arglist-style
  macro family in one mechanism.
- The heuristic remains best-effort: an author whose arglist uses unconventional
  parameter names gets a `Weak`/partial spec, never a fabricated one — same
  contract as ADR-0019.
- Both once-deferred derivations are now implemented: the Scheme `syntax-rules`
  pattern harvester (ADR-0031), and Clojure's `:arglists` metadata refinement.
  The latter reads the arglist from a `^{…}` reader-metadata map on the name or
  an attr-map before the arglist, classifies it, and — when it names known roles
  — overrides the raw parameter-vector spec with `Confidence::Declared` (the
  analog of elisp `declare`), since an author-supplied `:arglists` is the
  authoritative call shape. This also motivated a robust arglist search (skip a
  leading docstring string and attr-map), which additionally fixes documented
  Clojure/Janet macros whose docstring precedes the arglist. `:style/indent` is
  also read, as a lower-priority fallback (the analog of elisp `(indent N)`): it
  names only the body boundary, so an integer `n` pads the leading roles to `n`
  `Other` slots and opens the body, and `:defn`/`:form` just open the body — but
  only when `:arglists` did not already pin a richer, role-named shape. The
  nested `[n …]` list/vector form is read by taking its head element as the
  form-level indent (the rest describe nested args, which name no roles).
- Only Emacs Lisp's builtins are bundled from harvesting; other families' bundled
  cores stay hand-authored (ADR-0020), with harvesting reserved for consumers'
  project-local macros.
