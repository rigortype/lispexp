# A faithful reader, not a validator: report structural diagnostics only, and accept a per-implementation superset

## Context

lispexp reads S-expression *structure* fault-tolerantly (ADR-0004) and reports
diagnostics as `ErrorKind` (ADR-0023). A recurring expectation — and a tempting
feature request — is that lispexp also *validate* the input against a specific
Lisp: reject an unknown reader tag, a malformed number, a keyword that isn't
legal in the dialect, a construct a given implementation's reader would refuse.
That expectation is at odds with several decisions already made piecemeal:
lispexp captures any `#tag` verbatim without a per-dialect whitelist (ADR-0011),
its number/symbol split is a coarse lexical shape check with a `Symbol` fallback
(never a numeric-tower validation), `Options::scheme()` reads R6RS `#vu8(…)` and
arbitrary `#foo(…)` as data, and version *conformance* is out of scope
(ADR-0029). In practice lispexp accepts a **superset** of what any single
implementation's reader would. This ADR states the resulting stance as one
non-goal, and — crucially — draws the line between the diagnostics lispexp *does*
report and the validation it does not.

## Decision

**lispexp is a faithful reader, not a syntax checker, validator, linter, or
conformance tool.** It reads source into a faithful, position-annotated tree and
does not certify that the input is valid in any particular Lisp implementation.

The dividing line is **structural vs. semantic**:

- **Structural diagnostics are always reported**, because they fall out of
  parsing itself: unbalanced/mismatched/unexpected delimiters
  (`ErrorKind::{UnclosedList, MismatchedDelimiter, UnexpectedDelimiter}`), a
  dangling reader-macro prefix/tag/label (`DanglingPrefix`/`DanglingTag`/
  `DanglingLabel`), a `.` with no tail (`DanglingDot`), a malformed token
  (`MalformedToken`), and the recursion cap (`DepthLimitExceeded`). These are
  surfaced through `Parsed::errors` (and `parse_form_at`), always on and free;
  `parsed.errors.is_empty()` is a usable "structurally clean" signal. This *is*
  the "point out missing parens" capability — no separate mode is needed or
  offered.
- **Semantic / conformance validation is a non-goal**: lispexp does not reject
  an unknown or dialect-foreign reader tag (`#foo(…)`, `#vu8(…)` under Scheme),
  does not validate the numeric tower (ambiguous atoms fall back to `Symbol`),
  does not check that a symbol/keyword is legal, and does not verify that a
  construct is valid in the target implementation. Such input is read
  faithfully as data.

**A stricter validator is a consumer layer, not a reader mode.** Because the
structural `ErrorKind` set is already exposed per top-level form (ADR-0023), a
consumer that wants a bracket checker, a "reject any error" gate, or a
dialect-aware linter composes it on top of `Parsed::errors` / `parse_form_at` —
the same reader-supplies-mechanism, consumer-owns-policy split used for
write-safety (ADR-0023). Baking dialect-semantic validation into the reader
would contradict this non-goal and the reader-only scope (ADR-0001).

## Considered options

- **Validate reader tags / numbers / keywords per dialect.** Rejected:
  contradicts verbatim tag capture (ADR-0011) and the coarse-by-design number
  split; turns the reader into a per-dialect conformance database it
  deliberately is not (ADR-0029), and pushes past reader-only scope (ADR-0001).
- **A dedicated "validation mode" flag on the reader.** Rejected: the only
  validation lispexp can do without dialect semantics — structural balance — is
  already always reported via `errors`. A flag would imply a semantic checker
  that isn't there.
- **Say nothing (leave the stance implicit).** Rejected: users reasonably
  assume a "reader for dialect X" rejects X-invalid input; the superset-accepting
  behavior is surprising unless stated, and a README non-goal plus this ADR make
  it explicit.

## Consequences

- The boundary is documented for consumers: structural problems come back in
  `errors`; anything dialect-semantic is theirs to check on top.
- lispexp stays small and dialect-agnostic in its core, consistent with
  ADR-0001 (reader-only), ADR-0011 (verbatim tags), ADR-0023 (structured errors
  + consumer-owned policy), and ADR-0029 (dialect identity).
- A future dialect-aware linter, if ever wanted, is a separate layer or crate
  over this reader — not a change to the reader's contract.
