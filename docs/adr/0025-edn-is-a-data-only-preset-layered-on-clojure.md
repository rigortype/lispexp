# EDN is a first-class data-only preset layered on Clojure

## Context

Consumers such as lisplens infer a project's dialect, source roots, and
dependencies by parsing repo-local manifests with lispexp itself (lisplens
ADR-0015): `.asd`, `Cask`/`Eask`/`Eldev`, `Akku.manifest`, `clpmfile`,
`info.rkt`, and `deps.edn`. `deps.edn` is **EDN** — a data subset of Clojure —
not full Clojure. lispexp had presets for the Lisps involved but no EDN preset.
`Options::clojure()` is a syntactic *superset* of EDN (it also reads `#(`
anonymous functions, `#'` var-quote, `#?` reader conditionals, `#"…"` regex, `@`
deref), so a well-formed `deps.edn` already parses under it; the question is
whether EDN deserves its own preset.

## Decision

**Add a first-class `Options::edn()` preset that layers on `clojure()` (as Phel
does) with the code-only reader syntax turned off:** `#(` anonymous functions,
`#'` var-quote, `#?`/`#?@` reader conditionals, `#"…"` regex literals, `@`
deref, `'` quote, the `` ` ``/`~`/`~@` syntax-quote family, and `^` metadata are
all disabled — the EDN spec defines none of them. Tagged elements (`#inst`,
`#uuid`, user tags), `#{}` sets, and `#_` discard stay on, since they are valid
EDN. A valid `deps.edn` reads identically under `edn()` and `clojure()`; the
preset makes the intent explicit and lets validation-minded consumers reject
Clojure-only syntax in a data file.

Namespaced maps (`#:ns{…}`, already read as a tagged-literal marker on the
following map) also stay on — a pragmatic accommodation, not a spec claim:
they are Clojure 1.9 reader syntax that the edn-format spec text never adopted,
but `clojure.edn` accepts them and they appear in real `.edn` files in the wild.

> **Amended 2026-07-02:** the original decision disabled only the five
> `#`-flavored/`@` code forms and asserted namespaced maps "are valid EDN
> data". An audit found both wrong: `'`/`` ` ``/`~`/`~@`/`^` are equally
> non-EDN and are now disabled too, and the namespaced-map claim is corrected
> to "accepted by `clojure.edn`, absent from the spec".

**`info.rkt` needs no change.** `Options::racket()` already sets
`lang_line: true`, so a leading `#lang info` is captured as the lang line and the
remainder reads as Racket data. The restricted `#lang info` grammar
interpretation stays the consumer's concern.

## Considered options

- **Document `clojure()` as a safe superset, add no preset.** Rejected: EDN is a
  genuinely distinct, widely-used data language (Datomic, shadow-cljs, generic
  `.edn`), not merely Clojure config; a superset reader cannot reject malformed
  EDN and does not name the intent. Kept cheap to revisit, but the preset is the
  honest model.

## Consequences

- Preset addition is cheap under the Options-delta design (ADR-0003, ADR-0006):
  a few field overrides on `clojure()`, plus a `Dialect::Edn` variant and its
  `for_dialect` mapping.
- Consumers scanning manifests can name `deps.edn` as EDN and get a reader that
  parses data and rejects code-only syntax.
- Consistent with reader-only scope (ADR-0001): `edn()` still does not evaluate
  tagged literals or reader functions; it only declines to *read* code syntax.
