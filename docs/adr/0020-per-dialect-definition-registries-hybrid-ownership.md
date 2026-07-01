# Per-dialect definition registries: bundle a conservative core, let consumers own the long tail

## Context

Consumers such as lisplens ([ADR-0013 of lisplens]) and cccc need polyglot
definition annotation — "is this form a definition, and what is its name/kind?"
— across Common Lisp, Scheme, Clojure, and the rest of the dialect matrix, not
just Emacs Lisp. ADR-0019 built the annotator as a dialect-agnostic *mechanism*
(Registry / FormSpec / Role) but deliberately populated only elisp, and only via
harvesting (which is elisp-specific). The open question is who ships the
hand-authored def-form tables for the other ~10 dialects: lispexp, or each
consumer.

## Decision

**Hybrid, consumer-extensible (leaning toward lispexp owning more over time).**
lispexp bundles a *conservative, high-confidence core* Registry per dialect —
only the def-forms nobody disputes (`defun`/`defmacro`/`defvar`/`defclass`,
Scheme `define`/`define-syntax`, Clojure `defn`/`def`/`defprotocol`, …) — behind
the same feature gate as the annotator module (`lispexp::annotate`), and exposes a
`Dialect → default Registry` accessor plus a builder so consumers can extend and
override. The bundled specs are convenience; the authoritative source of truth
for a project's long tail (project-local def-macros, contested classifications)
stays with the consumer's own registry, composed on top of the bundled core.

**Kind is a raw head symbol plus an optional, conservative `category` hint.**
Every annotated definition always carries its verbatim head symbol
(`"defun"`, `"defn"`, `"defmethod"`); this is the reader-only, always-faithful
value. A FormSpec *may* additionally carry a normalized `category` hint
(function / macro / variable / class / method / …) — but only where the mapping
is uncontested. Ambiguous forms (e.g. Clojure `def`, which may bind a value or a
function) carry no category; the consumer classifies from the head if it wants.

## Considered options

- **lispexp bundles everything (full ownership).** Rejected *for now*: it makes
  lispexp the arbiter of "what counts as a definition" in every dialect and ties
  the opinionated, growing dataset to lispexp's release cadence. Kept on the table
  — real usage in lisplens/cccc will show how much bundling actually pays off,
  and we may move toward it.
- **lispexp ships only the mechanism; consumers own all tables.** Rejected:
  forces every consumer to re-type the same `defn`/`define`/`defun` tables that
  are small, well-known, and hand-enumerable.
- **lispexp owns a typed kind enum as the sole classification.** Rejected: a
  cross-dialect kind taxonomy breaks down at the edges (Clojure `def`,
  context-sensitive Scheme `define`) and pushes lispexp past the reader-only wall.

## Consequences

- Consistent with reader-only scope (ADR-0001) and ADR-0019: lispexp interprets
  declared/hand-authored structural metadata, never evaluates or classifies
  semantically beyond safe hints.
- The bundled core is a curated dataset lispexp must maintain — kept deliberately
  small so the maintenance and bikeshedding surface stays low. The degree of
  bundling is expected to grow as real use cases justify it.
- Consumers get correct results out of the box for the common forms and retain
  full control over the long tail via override/extend.
