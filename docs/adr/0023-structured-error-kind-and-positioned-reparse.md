# Structured `ErrorKind` and a positioned single-form reparse; the error diff stays with the consumer

## Context

lisplens's write safety (lisplens ADR-0005) is "never make a file's syntax
worse": after an edit it re-parses and blocks only if the edit *adds* parse
errors, using the pre-edit errors as a baseline (lispexp is fault-tolerant, so
files may already contain errors). Two obstacles: `ParseError` currently carries
a free-form `message: String`, so equality is stringly-typed and fragile; and an
edit shifts byte offsets, so a naive whole-file span-equality diff reports every
pre-existing error after the edit point as "new."

## Decision

**Replace `message: String` with a structured `ErrorKind`.** `ErrorKind` is a
`#[non_exhaustive]` enum covering the reader's diagnostics (unclosed list,
mismatched/unexpected delimiter, malformed token, dangling prefix/tag/label, …).
Variants may carry **non-positional** payload that sharpens identity and
diagnostics (e.g. `MismatchedDelimiter { expected, found }`) but never embed a
`Span`-derived value, so a kind stays shift-stable. The human-readable message
is produced by `Display`, not stored. `#[non_exhaustive]` lets us add kinds
without a breaking change.

**lispexp owns the mechanism; the "newly-introduced" diff stays in the consumer.**
Deciding what counts as a new error requires the edit geometry (offset, old/new
length), which is the consumer's edit-model concern (lisplens ADR-0005), not the
reader's. lispexp instead provides a positioned reparse primitive —
`parse_form_at(source, start) -> (Datum, Vec<ParseError>, end_offset)` — that
reads exactly one top-level form at/after `start` and reports where it ended,
with **spans absolute into the original `source`**. Because recovery is
top-level-granular (ADR-0004), an edit's effect is confined to the form(s) it
falls in, so a consumer re-validates just those forms and compares their small
`ErrorKind` sets locally — sidestepping the whole-file position-shift problem
without any shift-robust "normalized position" scheme.

## Considered options

- **lispexp owns an edit-aware `diff(before, after, edit)`.** Rejected: needs the
  edit geometry, pushing edit-model policy into the reader.
- **A flat category-only `ErrorKind`.** Rejected: non-positional payload
  (`expected`/`found`, tag string) improves both diff granularity and messages
  at no cost to shift-stability.
- **A full incremental parser with edit tracking.** Rejected: a large subsystem,
  and edit tracking is the consumer's domain.
- **Whole-file diff with a normalized position.** Rejected: designing a
  shift-robust position identity is hard and unnecessary once reparse is
  region-scoped.

## Consequences

- Dropping `message: String` is a breaking change to the public `ParseError`;
  worth it for comparable, hashable errors.
- Consumers get cheap validate-then-write on large files via `parse_form_at`,
  and keep full control of the accept/reject policy.
- Consistent with reader-only scope (ADR-0001) and top-level recovery
  granularity (ADR-0004).
