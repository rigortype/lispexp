# Hash-literal tags are open/unvalidated; datum labels get dedicated variants without graph resolution

Two related reader-syntax families need a representation. (1) **Hash literals** (`#(...)`, `#u8(...)`, `#hash(...)`, `#px"..."`, `#M(...)`, `#S(...)`, Clojure/Phel `#inst`/`#uuid`/custom tags): lispexp accepts any `#<tag>`-shaped form structurally into a `HashLiteral { tag, inner }` **without** validating the tag against a per-dialect whitelist, because robustness and future-proofing (R7RS-large, `register-tag`, custom reader tags) matter more than early rejection, and the requirements explicitly say the reader must not choke on dialect prefixes and must let the consumer identify/skip them. Consumers validate tags if they care. (2) **Datum labels** (`#n=<datum>` definition and `#n#` reference, in Scheme/Common Lisp/Racket) get dedicated `Label { id, inner }` and `LabelRef { id }` variants so consumers can skip them as data, but lispexp does **not** resolve the graph (reconstruct cycles/sharing) — that is a semantic operation excluded by the reader-only scope (ADR-0001).

> **Amended 2026-07-02:** the "consumers identify/skip tagged forms as one
> unit" promise was not actually discharged at the lexer layer until this
> refinement. An unrecognized `#tag(` used to lex as a separate `Symbol`
> (or hash-atom) token followed by a `List` open, splitting the tagged form
> in two — `#hash((a . 1))`, `#3a((1)(2))`, `#s(hash-table)`, and SRFI-4
> `#f64(1 2)` all split this way. The lexer now recognizes when a `#`-atom
> body runs directly into an active open delimiter and emits one `HashOpen`
> token covering the whole `#tag(` (or `#tag[`/`#tag{`), so the reader builds
> a single `HashLiteral { tag, inner }` as this ADR always intended.
>
> Relatedly, radix-prefixed numbers of the shape `#Nr...` (`#36rHELLO`,
> `#2r1010`) are classified as `Number`, not folded into the generic
> `HashLiteral` path — they are lexically numbers, not tagged data, and this
> keeps their classification consistent with the "shape, not meaning" rule
> from ADR-0016/lexical-shape classification.
