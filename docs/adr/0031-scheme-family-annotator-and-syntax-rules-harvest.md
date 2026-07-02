# Scheme-family annotation: no Emacs-style declared metadata; expand the bundled core, and harvest `syntax-rules` patterns

## Context

The definition-form annotator (ADR-0019) is a dialect-agnostic *mechanism*
(Registry / FormSpec / Role) with a *harvester* that is Emacs-Lisp-specific: it
derives specs from Emacs's machine-readable metadata — the Edebug
`(declare (debug (&define …)))` spec, `(declare (doc-string N) (indent N))`, and
— highest-yield in the wild — the def-macro's own arglist parameter names. The
open question (raised while looking beyond Emacs Lisp, starting with the Scheme
family: Gauche and the SRFIs): does Scheme carry an equivalent machine-readable
source a harvester could scan?

Findings:

- **No Edebug/`declare` analog exists.** Neither R7RS, R6RS, nor the SRFIs
  standardize a declaration that describes a macro call's argument structure to
  tooling. There is no `(declare (debug …))`, no `(doc-string N)`, and Scheme's
  core has no docstring concept at all (hence `scheme_builtins` is `NoDoc`).
- **Indentation/editing metadata lives outside the Scheme source.** It is held
  by editors — Emacs `scheme.el` / `gauche-mode.el` via the
  `scheme-indent-function` symbol property, DrRacket's own tables — not by the
  Scheme program, so scanning `.scm` yields nothing here (and ADR-0022 already
  scopes indent harvesting as Emacs-Lisp-specific).
- **The SRFIs are a map of macro *mechanisms*, not a metadata standard.** The
  `?q=macro` set (SRFI 46/148/211 `syntax-rules` extensions, SRFI 93
  `syntax-case`, SRFI 139 syntax parameters, SRFI 213 identifier properties,
  SRFI 188/206/212/219 binding/alias/definition forms) tells us *which*
  transformer kinds exist — i.e. which are harvestable and which are not — but
  none standardize structural-role metadata. SRFI 213 Identifier Properties is
  the closest to Emacs symbol plists, but it is an **expand-time** facility, not
  a statically-readable annotation, so it is out of reach for a reader-only tool
  (ADR-0001).
- **The `syntax-rules` *pattern itself* is the harvestable signal**, and it is
  the direct analog of — and richer than — Emacs's "def-macro arglist parameter
  names" heuristic. In
  `(define-syntax define-test (syntax-rules () ((_ name (arg …) body …) …)))`
  the input pattern `(_ name (arg …) body …)` names the argument roles *and*
  shows their nesting and ellipsis (variadic) structure, which a flat elisp
  arglist cannot. The pattern variable names are author-chosen (same limitation
  as the elisp heuristic → `Confidence::Inferred`), and the signal is absent for
  procedural transformers (`syntax-case`, `er/rsc/sc-macro-transformer`, Gauche
  legacy `define-macro`), exactly as `&define` is absent for elisp macros that
  "do arbitrary things."

## Decision

Two moves, both consistent with reader-only scope (ADR-0001) and the
conservative-core / consumer-extensible ownership model (ADR-0020):

1. **Expand the bundled Scheme-family core with hand-authored, uncontested
   specs**, keeping strict `Scheme` at R7RS and layering implementation-common
   forms onto the extended dialects (mirroring how `racket_builtins` layers on
   `scheme_builtins`):
   - **Strict `Scheme` (R7RS):** add `define-library` (name only, no category)
     to the existing `define` / `define-values` / `define-syntax` /
     `define-record-type`.
   - **Extended family (`Guile`, `Gauche`, `Mosh`, `Gambit`, `SchemeSuperset`):**
     layer on the R7RS core the forms shared by Guile GOOPS and Gauche and
     common `.scm` code — `define-class` (Class), `define-generic` (Generic),
     `define-constant` (Constant), `define-inline` (no category, `define`-like),
     `define-syntax-rule` (Macro), and Guile's `define*` / `define-public` (no
     category). Bundling these in the superset is safe: a spec matches only when
     its head appears, so a form absent from Mosh/Gambit simply never fires
     (ADR-0030, faithful-reader-not-a-validator).
   - **`Racket`:** add `define-syntax-rule` (Macro) to its existing core.
   - **Deliberately excluded:** `define-method`. Its structure is *not* uniform
     across the family — Gauche writes `(define-method name (specialized-args)
     …)` (name is a symbol) while Guile GOOPS writes `(define-method (name
     (arg <class>) …) …)` (name is nested inside the arglist list). Bundling one
     shape would mis-read the other, so it stays for a consumer to supply
     (ADR-0020) rather than fabricate a cross-dialect-inconsistent spec.

2. **Plan a `syntax-rules` pattern harvester** as the Scheme-family counterpart
   to the elisp arglist heuristic (the mechanism `annotate_form` is already
   dialect-agnostic, so only the *derivation* is new): detect
   `define-syntax` + `syntax-rules`, take each rule's input pattern, and map its
   pattern-variable names through the same small role vocabulary the elisp
   harvester uses (`name`→Name, `arg`/`args`→Arglist, `body`→Body, …), emitting
   `Confidence::Inferred` specs. Procedural transformers yield no pattern and are
   left to weak signals only (naming conventions, template-expands-to-`define`).

   **Implemented** in `harvest_scheme_macro`, wired into `harvest_source_for`
   for the Scheme family (ADR-0032). It reads each rule's input pattern, treats a
   nested list as an `Arglist`-shaped sub-pattern and a trailing `X …` ellipsis
   as the body tail, keeps the richest rule of a multi-rule macro, and skips
   procedural transformers (`er-macro-transformer`, a lambda with no pattern, …).
   Coverage widened past bare `syntax-rules` to the other pattern-carrying forms:
   `define-syntax-rule` (Racket/Guile), a `syntax-case` / `syntax-parse`
   transformer found anywhere in the transformer expression (including inside a
   `lambda`, with `name:id` syntax-class suffixes stripped), and the legacy
   non-hygienic `define-macro` (Guile/Gauche/Gambit), whose signature is an
   ordinary arglist with a dotted tail as the body.

## Consequences

- Scheme-family code annotates for the common definition forms out of the box,
  with strict `Scheme` staying R7RS-faithful and the extended dialects gaining
  implementation-common forms — no new reader surface, no evaluation.
- The bundled dataset grows (ADR-0020 anticipated this); it stays small and
  uncontested, and the structurally-divergent `define-method` is explicitly a
  consumer concern with the reason recorded.
- A future `syntax-rules` harvester reuses the existing annotator mechanism and
  role vocabulary; only `syntax-rules`-defined macros are in reach, and their
  inferred specs carry `Confidence::Inferred`, never fabricating structure from
  procedural transformers.
