# A fixed recursion-depth cap keeps `parse` panic-free on pathologically nested input

## Context

`parse` is documented to never panic, but the reader is a recursive-descent
parser: each nested list or hash literal descends one more
`finish_list`/`parse_datum` stack frame. Nothing bounded that recursion.
Pathologically nested input — `(((((...` repeated tens of thousands of times,
whether handwritten, fuzzed, or machine-generated — overflowed the stack and
crashed the process, violating the "never panics" contract (ADR-0004).
Empirically, an unoptimized (debug) build overflows an 8 MB default-sized
stack around roughly 1,800 levels of nesting; a 2 MB thread, such as Rust's
default test-harness thread, overflows around ~450 stack frames, because each
frame in a debug build is large (a `Datum` plus several locals). Any fix has
to hold on the smallest realistic stack, not just the default one, since
consumers commonly run parsing on spawned threads with a reduced stack size.

## Decision

**Cap nesting depth at a fixed `MAX_DEPTH = 200`.** A depth counter threads
through `finish_list`/`finish_hash`; once it is exceeded the reader stops
descending into the too-deep region, reports `ErrorKind::DepthLimitExceeded`
once, and skips the balanced region as a unit — the same fault-tolerant
pattern as every other recovery case in ADR-0004: a malformed/pathological
region loses only itself, and prior siblings and everything after the
skipped region are kept. 200 is chosen with a wide safety margin below the
~450-frame empirical failure point on a 2 MB thread, so `parse` genuinely
never panics regardless of how deep the input nests, tested up to 100,000
open parens.

## Considered options

- **An iterative parser with an explicit heap-allocated stack.** Rejected:
  it would remove the recursion limit entirely (bounded only by memory), but
  at a real complexity cost — the recursive-descent structure is easy to
  read and maintain, and no real Lisp source, generated or handwritten,
  actually nests deep enough to need an unbounded parser. Revisiting this is
  possible later without an API break, since callers only observe
  `DepthLimitExceeded` and a skipped region either way.
- **A configurable depth cap on `Options`.** Rejected for now as YAGNI: no
  concrete dialect or consumer need has surfaced for a different limit, and
  `Options` is already `#[non_exhaustive]`, so a `max_depth` field (or a
  dedicated setter) can be added later without a breaking change if a real
  need shows up.
- **No cap; document the panic as a known limitation.** Rejected: it
  contradicts the reader's core fault-tolerance promise (ADR-0004) and turns
  a diagnostic-shaped problem (deeply nested input) into an availability
  problem (process crash) for any consumer that parses untrusted or
  machine-generated input.

## Consequences

- Real Lisp code — handwritten or from the corpora this crate is tested
  against — nests well under 100 levels deep; the cap is invisible in
  practice.
- Machine-generated or adversarial files with extreme nesting now get a
  `ParseError { kind: DepthLimitExceeded, .. }` diagnostic and a partial tree
  instead of a crash, preserving the "never panics" contract from ADR-0004
  and this crate's broader fault-tolerance design.
- The cap is a fixed internal constant, not yet part of the public API
  surface; making it configurable later is additive, not breaking.
