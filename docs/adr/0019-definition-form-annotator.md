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
   `(declare (debug SPEC))` when SPEC leads with `&define`, `(doc-string N)`,
   `(indent …)`, the `doc-string-elt` property, **and — crucially for third-party
   macros — the macro's own arglist parameter names** (see "Heuristic sources"
   below) — and derives a **form spec**: a mapping from argument positions to
   roles (Name, Arglist, Docstring, Declare, Interactive, Body). Harvested specs
   accumulate into a **form-spec registry**. The known declaration keywords come
   from Emacs's own `defun-declarations-alist` / `macro-declarations-alist`.
2. **Form annotator** (tentatively "macro-annotator"): walks a `Datum` tree; for
   any list whose head symbol is in the registry, it matches the remaining
   children against that form's spec and tags them with their roles, so a
   consumer can read `(cl-defun foo (x) "doc" body…)` as name=`foo`,
   arglist=`(x)`, docstring=`"doc"`, body=`body…` without knowing `cl-defun`
   specifically. `annotate_tree`'s tree descent follows the code-vs-data walker
   (ADR-0026, via `code_nodes`) rather than a bespoke "recurse into lists" rule:
   a form is annotated iff it sits in *code* position, so a definition guarded by
   a reader/feature conditional (`#+sbcl (defun …)`) or unquoted inside a
   quasiquote is reached, while a quoted form or a quasiquote template is inert
   data and skipped. This keeps the annotator from re-deriving — and diverging
   from — the reader's own quote/quasiquote semantics.

## Heuristic sources (empirical, from `~/.emacs.d/elpa`)

Analyzing 531 top-level `defmacro`/`cl-defmacro` across 1227 third-party elisp
files showed that the authoritative `&define` signal is **sparse in the wild** —
only ~8 `(declare (debug (&define …)))` specs exist across the whole tree. The
harvester therefore cannot rely on `&define` alone and combines several
heuristic sources, in roughly descending precision:

1. **`(declare (debug (&define name lambda-list … def-body)))`** — authoritative,
   but rare in third-party code (common only in Emacs core).
2. **`(declare (doc-string N))`** — ~31 cases; a strong "this is a
   definition-like form" signal that also pins the docstring's argument position
   (`eask-defcommand`, `lsp-defcustom`, `dirvish-define-preview`, `defhydra`, …).
3. **The definition macro's own parameter names** — the highest-yield heuristic
   for un-annotated macros: a def-macro's arglist literally names its argument
   roles, e.g. `(cl-defmacro dirvish-define-preview (name &optional arglist
   docstring &rest body) …)` and `(defmacro defhydra (name body &optional
   docstring &rest heads) …)`. A small role vocabulary maps parameter names to
   roles (observed frequencies in parentheses):
   - **name**: `name` (38), `symbol`/`sym`/`fsym`/`fn-name`, `var`/`variable`,
     `place`, `target`
   - **arglist**: `args` (20), `arglist` (8), `arguments`, `lambda-list`, `key-args`
   - **docstring**: `docstring` (13), `doc` (12)
   - **body**: `body` (220), `forms`, `bodyform`, `def-body`
   - (`varlist`, `clauses`, `then-form`/`else-forms` instead signal *binding* or
     *control* macros — useful to classify the macro's kind, not annotate a def)
4. **`(indent N)`** — ~349 cases; `N` approximates the count of leading
   non-body ("distinguished") arguments, helping locate where the body begins
   (e.g. `defun` = `(indent 2)`: name + arglist before the body).
5. **Naming conventions** (`def*`, `define-*`, `*-def*`) and **body expands to a
   definition** (the macro's backquoted template contains `defun`/`defalias`/
   `defvar`/`cl-defstruct`) — weak corroboration only.

Each harvested spec carries the signals that produced it (provenance) and a
confidence; the annotator applies only specs above a threshold and never
fabricates structure from a weak signal alone.

**Bundled builtins.** GNU Emacs's own definition-macro specs are harvested from
the Emacs source tree (checked out at `/Users/megurine/local/src/emacs`) and
shipped as a generated built-in registry, so lispexp recognizes the standard
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
  a feature-gated module (e.g. `lispexp::annotate`) or sibling.
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
