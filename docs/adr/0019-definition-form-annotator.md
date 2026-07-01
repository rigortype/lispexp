# Definition-form annotator: a utility layer that tags def-forms via declared specs

## Context

Consumers such as `cccc` need to find *definitions* in a parsed tree — functions,
macros, variables — and locate their parts (the defined name, the arglist, the
docstring, the body) to emit IR like `Node::Function`. In Emacs Lisp this cannot
be done by hard-coding head symbols: `defun` and `defmacro` are themselves macros
(not special forms), and countless definition forms are macro-provided —
`cl-defun`, `cl-defmacro`, `define-minor-mode`, `define-derived-mode`,
`cl-defstruct`, `ert-deftest`, and project-local macros. Enumerating them by hand
does not scale and misses project-specific ones.

Emacs already carries machine-readable structure for these forms. A definition
macro declares its shape via `(declare (debug SPEC) (doc-string N) (indent …))`,
where the Edebug `debug` SPEC is a grammar of the argument structure. Crucially,
definition forms lead their spec with **`&define`**, and use structural keywords
that name roles directly: `name` (the defined symbol), `lambda-list` (the
arglist), `stringp` / `lambda-doc` (the docstring), `def-body` / `body` (the
body), plus `[&optional ("declare" …)]` and `[&optional ("interactive" …)]`. For
example `cl-generic.el`: `(declare (debug (&define name lambda-list def-body))
(indent defun))`; and `byte-run.el` records `(function-put 'defmacro
'doc-string-elt 3)`. So "a form whose debug spec begins with `&define`" *is* a
definition form, and its spec tells us the role of each argument position.

## Decision

Build a **definition-form annotator** as a utility layer that sits on top of the
reader's `Datum` tree — parallel to the parser, not part of it — in two parts:

1. **Spec harvester** (tentatively "macro-collector"): scans Emacs Lisp source,
   reads each definition macro's `declare` metadata — chiefly the Edebug
   `(declare (debug SPEC))` when SPEC leads with `&define`, plus `(doc-string N)`
   and `(indent …)` and the `doc-string-elt` property — and derives a **form
   spec**: a mapping from argument positions to roles (Name, Arglist, Docstring,
   Declare, Interactive, Body). Harvested specs accumulate into a **form-spec
   registry**. The known declaration keywords come from Emacs's own
   `defun-declarations-alist` / `macro-declarations-alist`.
2. **Form annotator** (tentatively "macro-annotator"): walks a `Datum` tree; for
   any list whose head symbol is in the registry, it matches the remaining
   children against that form's spec and tags them with their roles, so a
   consumer can read `(cl-defun foo (x) "doc" body…)` as name=`foo`,
   arglist=`(x)`, docstring=`"doc"`, body=`body…` without knowing `cl-defun`
   specifically.

**Bundled builtins.** GNU Emacs's own definition-macro specs are harvested from
the Emacs source tree (checked out at `/Users/megurine/local/src/emacs`) and
shipped as a generated built-in registry, so sexpp recognizes the standard
def-forms out of the box. The harvester can additionally scan a user's project
directory to extend the registry with project-local definition macros.

**Naming.** Prefer **"form annotator"** and **"spec harvester"** over the
tentative "macro-annotator" / "macro-collector": the prefix "macro-" wrongly
suggests macro-*expansion* (an ADR-0001 non-goal), whereas this subsystem never
runs or expands macros — it reads *declared structural metadata* and tags the
tree. The shared data is a **form spec** / **form-spec registry**.

## Consequences

- **Consistent with reader-only scope (ADR-0001).** No evaluation or
  macro-expansion happens; the annotator only interprets declared metadata. It is
  an optional module built on the tree, keeping the reader core untouched — likely
  a feature-gated module (e.g. `sexpp::annotate`) or sibling.
- **Heuristic / best-effort.** `declare`/Edebug specs are advisory and
  incomplete: some macros lack a spec, some do arbitrary things. The annotator
  annotates what it can confidently recognize and leaves the rest un-annotated;
  it must never guess in a way that fabricates structure.
- **Harvesting is Emacs-Lisp-specific; annotation is dialect-agnostic.** The
  `declare`/Edebug harvester is elisp-only, but the annotator mechanism (apply a
  role spec to a tree) is general and can later be driven by hand-written specs
  for other dialects' def-forms (Clojure `defn`, Scheme `define`, CL `defun`),
  serving the same `cccc` need across the dialect matrix.
- The Emacs source dependency is a *build/dev-time* input (to generate the
  bundled registry), not a runtime dependency, preserving the pure-Rust,
  dependency-light posture (ADR-0013).
