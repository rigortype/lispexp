# Dispatch forms: variable-length qualifier role, separate Clojure dispatch value, and structured specializer exposure

## Context

Consumers disambiguate same-named methods by a syntactic *dispatch signature*
(lisplens ADR-0009): CL/elisp `(cl-defmethod foo :around ((x integer)) …)` →
name `foo`, qualifier `:around`, specialized arglist `((x integer))`; Clojure
`(defmethod area :circle [shape] …)` → dispatch value `:circle`. lispexp today
registers `cl-defmethod` as a fixed `[Name, Arglist]` spec, which mis-reads
`:around` as the arglist and exposes no specializers. The FormSpec vocabulary
from ADR-0019 is fixed-position and cannot express "zero-or-more optional
qualifiers between the name and the arglist."

## Decision

**A variable-length `Qualifiers` role plus a separate `DispatchValue` role.**
Extend the FormSpec vocabulary with a greedy, variable-arity `Qualifiers*` role:
after `Name`, it consumes every child up to the first *delimited list*
(`( )`/`[ ]`), which is taken as the arglist boundary — so CL/elisp methods with
zero, one, or several qualifiers all annotate correctly. Boundary detection uses
only the token *shape* (is-a-delimited-list), never type resolution, consistent
with reading tokens verbatim. Clojure's `defmethod` is a different animal — its
`:circle` is an arbitrary single dispatch datum, not a qualifier — so it gets
its own `DispatchValue` role (exactly one Datum), not `Qualifiers`.

**Specializers are exposed as structured `(var, specializer)` pairs, verbatim.**
The arglist of a method carries a `SpecializedArglist` role with an accessor
that splits each required parameter into `(variable-token, Option<specializer
Datum>)`. Specializers are returned as raw Datums — a symbol Datum for
`integer`, a list Datum for `(eql form)` — and lispexp does **not** interpret
them: it does not resolve types, does not evaluate the `form` in `(eql form)`,
and does not even flag "this is an eql specializer." It only splits the pairs.

## Considered options

- **Single optional `Qualifier?` (at most one).** Rejected: elisp and most code
  use one qualifier, but ANSI CL allows several; capping at one contradicts
  lispexp's faithful-reader posture and widening it later would be breaking.
- **Reuse one "dispatch" role for both CL qualifiers and Clojure dispatch
  values.** Rejected: they mean different things (a modifier keyword vs. an
  arbitrary value); conflating them loses information.
- **lispexp builds the dispatch-signature string itself.** Rejected: how to order
  qualifiers, render `…`, and unify CL with Clojure is a consumer-specific
  opinion (lisplens ADR-0009), outside lispexp's reader-only remit.
- **Tag `SpecializedArglist` only, no pair-splitting.** Rejected: every consumer
  would re-implement `(var spec)` decomposition.

## Consequences

- Covers the `cl-defmethod` / CL `defmethod` / `defgeneric` / Clojure
  `defmethod` family with one small vocabulary extension.
- lispexp hands back verbatim tokens with structure; all *meaning* (what the
  specializer resolves to, what the signature string looks like) stays with the
  consumer — consistent with reader-only scope (ADR-0001).
