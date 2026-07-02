# LineIndex reports byte columns; char/UTF-16 columns are the consumer's to derive

## Context

Line-hash mode and diagnostics in consumers like lisplens are line-centric and
report line/column, so they need a line↔byte mapping independent of the Datum
tree. lispexp stores byte-only `Span`s and attaches a 1-based line per Datum, with
column "derived on demand" but no whole-file index exposed. Exposing a public
`LineIndex` over a `&str` raises the question of what a "column" is measured in
— bytes, Unicode scalars, or LSP's UTF-16 units.

## Decision

Add a public `LineIndex` over `&str` with `offset_to_line_col`,
`line_col_to_offset`, and `line_range(n)`. **Columns are byte offsets from the
start of the line.** Consumers that need char or UTF-16 columns derive them by
slicing `source[line_start..offset]` (via `line_range`) and counting — lispexp
does not own encoding conversion (ADR-0017).

Conventions, chosen to match the existing 1-based Datum line and Rust's
`str::lines`:
- **line and column are 1-based** (LSP's 0-based conversion is the consumer's);
- line breaks are **`\n` and `\r\n` only** — a lone `\r` is not a break;
- **`line_range(n)` excludes the terminator** (line content only); an offset on
  the newline maps to the column just past the last content byte.

> **Amended 2026-07-02.** lisplens (a verbatim/round-trip consumer) flagged that
> a uniformly byte-oriented API whose natural "line N" accessor silently returns
> *normalized* content is a footgun: `line_range`'s ranges do not tile the source
> (terminator bytes belong to no range) and give no way to recover a line's
> verbatim bytes or its terminator kind. `line_range` stays as the content/hash
> default, and two additive, non-breaking accessors make the verbatim path
> discoverable: **`line_full_range(n)`** (content *and* terminator — these ranges
> tile the source and reconstruct it exactly) and **`line_terminator(n) ->
> Terminator { Lf, CrLf, None }`** (the break kind). `Terminator` is a closed
> set, matched exhaustively like `Class` (ADR-0026), since the `\n`/`\r\n`-only
> line policy above is fixed.

## Considered options

- **Char (Unicode scalar) columns.** Rejected: closer to human display but still
  not what LSP wants, and it makes lispexp count encoding units.
- **LSP-compatible UTF-16 columns.** Rejected: pulls encoding conversion into the
  reader, contradicting ADR-0017; UTF-16 is an editor-layer concern the consumer
  maps.
- **Recognize a lone `\r` as a line break.** Rejected: negligible in Lisp source
  and complicates the line scanner for no real benefit.

## Consequences

- `LineIndex` is exact, cheap, and encoding-agnostic; byte columns +
  `line_range` are a complete minimal basis from which any column unit is
  derivable.
- Consistent with the byte-oriented `Span` (ADR-0008) and encoding-as-consumer-
  concern (ADR-0017).
