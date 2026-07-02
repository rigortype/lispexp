# Scheme dialect triage: Gauche's reader extensions

`cccc-scheme` targets **R7RS-small** (see the crate's module doc). This note
records what happens when it meets real-world Scheme code that goes beyond
that — specifically [Gauche](https://practical-scheme.net/gauche/), a widely
used, R7RS-superset implementation — and the architectural reasoning behind how
`cccc` should (and should not) react to it.

## What was audited

`cccc --lang scheme` was run against three corpora already used for the R7RS
work (`chibi-scheme`, `lem`, `typed-racket`, all under
`lispexp`'s own test corpus) plus a fourth: a full local checkout of
[Gauche](https://github.com/shirok/Gauche) at `/Users/megurine/repo/scheme/Gauche`.

| Corpus | Files | Functions | Parse errors |
|---|---:|---:|---:|
| chibi-scheme (R7RS reference impl) | 609 | 7,676 | **0** |
| lem (`.scm` = tree-sitter queries, not Scheme) | 7 | 0 | 0 (expected: no `define`s) |
| typed-racket (Racket, not R7RS) | 5 | 6 | 0 |
| **Gauche** | 888 | 12,157 | **288** (40 files) |

chibi-scheme — the actual R7RS-small reference implementation — parses
perfectly: 609 files, 0 errors. That's the strongest signal that the R7RS-small
mapping itself is sound. Gauche is the outlier.

## Root cause

Gauche extends R7RS's `#`-prefixed read syntax with two forms `lispexp` (and
R7RS) don't know:

- **`#[...]`** — a char-set literal, e.g. `#[\(\[\{]`. 64 files use it.
- **`#/regexp/`** — a regexp literal, e.g. `#/[\\\"]/`. 146 files use it.

Both can contain raw `(`, `[`, `\`, `/` bytes that aren't meant to be read as
Scheme delimiters at all — they're opaque payload up to the matching `]` or the
next unescaped `/`. `lispexp`'s reader doesn't special-case `#[` or `#/`, so it
starts tokenizing *inside* the literal as ordinary code, immediately
mismatches a bracket, and errors.

## Impact: this cascades, unlike the typed-racket case

The `case-lambda`-as-type-constructor false positive found earlier in
typed-racket was **contained** — one spuriously-reported unit, everything else
in the file unaffected — because it's a case of the reader syntax being
*understood* but the *semantics* (code vs. type-level use) being ambiguous.

The Gauche case is different in kind: the reader **loses sync** at the `#[`/`#/`
token and doesn't reliably resynchronize at the next top-level form. Of the 40
affected files:

- 2 lose **100%** of their functions (`benchmark.scm`, `prof.scm` — small files
  where the bad literal appears early, so there's no "before" to salvage).
- The rest keep a partial function count (e.g. `array.scm`: 971 lines, 14
  functions recovered despite 10 error messages), but some fraction of
  legitimate, ordinary R7RS-small code in the same file is silently lost.

This is surfaced to the user via `parse_errors` in the JSON/table output (the
existing fault-tolerance contract every adapter honors), so it isn't
*invisible* — but losing a whole small file's worth of otherwise-valid
complexity data because of one regexp literal is a real quality gap for anyone
who points `cccc` at a Gauche codebase, which is common in practice (Gauche is
one of the most widely deployed Scheme implementations, especially in Japan).

## Why "detect Gauche, then switch the whole project to a Gauche mode" doesn't fit `cccc`'s architecture

The idea considered: have `cccc`, on encountering a `.scm` file it can't parse
as strict R7RS, infer "this project is actually Gauche" and re-analyze the
*rest* of the project's `.scm` files in a Gauche-tolerant mode.

This does not fit cleanly, for three independent reasons in the current
design (`crates/cccc-cli/src/lib.rs`):

1. **`AnalyzeFn` is a bare, stateless function pointer** —
   `pub type AnalyzeFn = fn(&Path, &str) -> FileReport`. There is no closure
   capture, no `&mut self`, nowhere to remember "we already saw a Gauche-only
   construct in file N" for file N+1 to consult. Every adapter (`cccc-es`,
   `cccc-rs`, `cccc-go`, `cccc-php`, `cccc-rb`, `cccc-scheme`) relies on this
   single-file-in/single-report-out contract — it's the seam that keeps each
   adapter a standalone library with no CLI dependency (see
   `docs/ADDING_A_LANGUAGE.md`).
2. **File analysis is parallel and order is not meaningful** — `cccc-cli`
   dispatches files via `par_iter()` (rayon) once the file count crosses a
   threshold. Which file "discovers" the Gauche-only construct first is
   non-deterministic across runs. A project-wide mode switch keyed on
   discovery order would make output depend on `--jobs` / scheduling, which
   `cccc-cli/tests/cli.rs`'s `jobs_option_produces_same_output` test explicitly
   guards against today for every language.
3. **Dispatch is a static, compile-time extension table** — `lang::LANGUAGES`
   maps an extension to one `analyze` function chosen before any file is read.
   Making that choice depend on *other files' content* would require a
   sequential pre-scan pass ahead of the existing walk-and-dispatch pipeline —
   a materially different pipeline shape, not a local change to one adapter.

In short: nothing about *detecting* Gauche syntax is hard (`#[`/`#/` are
trivially recognizable at the lexer level); it's specifically the **cross-file,
order-independent, stateless-function-pointer** constraints that rule out a
project-level dialect switch without a broader `cccc-cli` redesign.

## What does fit: a per-file, stateless "Scheme superset" tolerant mode

The practical goal — Gauche-flavored `.scm` files parse acceptably — is
achievable without any of the above, by making the *reader itself* tolerant of
`#[...]` / `#/regexp/.../` as opaque atoms (consumed and discarded, not
descended into) inside `cccc-scheme::to_ir`. This is:

- **Per-file and stateless** — no memory needed across files, so it composes
  with `par_iter()` and the existing `AnalyzeFn` signature unchanged.
- **A strict widening, not a mode switch** — R7RS-small code parses exactly as
  before; only the two previously-fatal token shapes stop being fatal. No
  flag, no config, no cross-file inference.
- **Bounded in scope** — recognizing two additional `#`-prefixed token shapes
  at the lexer boundary, not implementing Gauche's semantics (the resulting
  atom is treated as opaque data, same as any other literal `cccc-scheme`
  doesn't score).

This is the direction implemented next (see the corresponding commit in
`crates/cccc-scheme`).

## Result

`cccc-scheme::desugar_gauche_extensions` rewrites `#[...]` and `#/regexp/`
literals to a same-length run of `_` before parsing (skipping occurrences
inside a string, a line comment, or a nested `#| … |#` block comment, and
leaving an unterminated literal untouched so genuinely malformed input still
errors). Re-running the same Gauche checkout:

| | Files | Functions | Parse errors |
|---|---:|---:|---:|
| Before | 888 | 12,157 | 288 (40 files) |
| After | 888 | 12,486 | **3 (1 file)** |

329 more functions recovered; parse errors dropped from 40 files to 1.

### The one residual file

`src/srfis.scm` still reports 3 "unclosed list" errors — but not from `#[`/`#/`.
The script deliberately calls `(exit 0)` and puts free-form documentation text
*after* it in the same file (the comment above that line reads "we don't use
'main' in order to put the data after `(exit 0)` line"); the script reads that
trailing text itself, as data, at run time — it's never meant to be read as
Scheme syntax at all, by Gauche's own reader or anyone else's. This is a
"data after exit" idiom (the Scheme analogue of Perl's `__DATA__` /
Ruby's `__END__`), not a reader-syntax gap, and no static full-file reader can
special-case it without modeling `exit`'s runtime effect on reading order —
out of scope for `desugar_gauche_extensions` (and for a static-analysis tool in
general). Importantly, this doesn't cost anything: all 9 real functions earlier
in the same file (`parse`, `record-generator`, `generate`, …) are still
correctly detected — the 3 errors are fully confined to the trailing data
block, confirming the shim's "resync at the next top-level form" property
holds even for this unrelated failure mode.

## Superseded by `lispexp` 0.2.0's `Options::scheme_superset()`

`lispexp` 0.2.0 (2026-07-02) shipped exactly this fix at the reader level —
`Options::scheme_superset()` / `Dialect::SchemeSuperset` (ADR-0027 in the
`lispexp` repository), a tolerant `.scm` preset covering `#[...]`/`#/regexp/`
plus more of the shared Gauche/Mosh/Gambit surface (`#"..."` interpolated
strings, `#vu8(...)` bytevectors, both leading- and trailing-colon keywords).
`cccc-scheme` now depends on `lispexp` 0.2 and reads every `.scm`/`.ss`/`.sld`
file with `Options::scheme_superset()`; the hand-rolled
`desugar_gauche_extensions` shim described above was deleted, along with the
tests that exercised its internal scanning logic (POSIX-class nesting inside
`#[...]`, unterminated-literal fallback, …) — that lexical correctness is now
`lispexp`'s concern and covered by its own test suite. `cccc-scheme` keeps
only the handful of tests that confirm *its own* integration: that `to_ir`
actually requests the superset and correctly lowers what it returns.

Re-running the same Gauche checkout against the new reader reproduces the
numbers above **exactly** — 12,486 functions, 3 errors in 1 file — confirming
the two approaches are behaviorally equivalent for this corpus, as expected
from ADR-0027 citing the same audit.

### A negligible, expected side effect on chibi-scheme

Re-running the chibi-scheme corpus (the audit's other anchor) shows 7,672
functions instead of 7,676 — 4 fewer, all in `lib/chibi/syntax-case.scm`, all
`cognitive = 0`. This is **not a regression to work around**: `lispexp` 0.1.1
had a lexer bug where chibi's own `` #` ``/`#,`/`#,@` reader macros (a
syntax-case hygienic-template notation, unrelated to Gauche) weren't
recognized at all and were mis-tokenized in a way that accidentally "leaked" a
nested `lambda` as a sibling list element. `lispexp` 0.2.0 now correctly reads
these as `DatumKind::HashLiteral`, and — per `lispexp`'s own ADR-0026 (the
code-vs-data walker, landed the same release) — a `HashLiteral`'s contents are
classified as `Data` unconditionally, since a generic `#tag(...)` payload could
mean anything (from `#u8(...)` bytevector data to a macro-authoring
convention like this one) and `lispexp` deliberately doesn't guess per tag.
`cccc-scheme`'s own `lower_datum` follows the same stance (it only descends
into `List` and `Prefixed`, never `HashLiteral`), so those four
`lambda`s — a chibi-internal macro-implementation detail, not R7RS-small user
code — are no longer counted. The effect is confined to raw function *count*;
no reported cognitive/cyclomatic score anywhere in the corpus changed.

A cleaner long-term fix, not done here: `cccc-scheme`'s `lower_prefixed`/
`lower_quasi` hand-roll the same quote/quasiquote/unquote code-vs-data ruling
that `lispexp::walk` (ADR-0026) now provides as a first-class, officially
-maintained API. Migrating to `walk`/`Class` would let `cccc-scheme` track
`lispexp`'s own considered semantics (including cases like this one) instead
of maintaining a parallel, potentially-diverging judgment — worth a follow-up,
but a larger change than adopting 0.2.0 itself.

## Follow-up done: migrated to `lispexp::walk` (0.3.0)

`cccc-scheme` now depends on `lispexp` 0.3 (backward-compatible; no reader
behavior changed) and `Builder::lower_datum` — the single entry point every
other lowering function recurses through — now calls `lispexp::walk` directly
instead of hand-rolled `lower_prefixed`/`lower_quasi` functions (both
deleted). For each `Datum`, `walk` classifies every node as `Class::Code` or
`Class::Data`; `lower_datum`'s callback hands each `Code`-classified `List` to
the existing `lower_list` (unchanged — all the special-form/complexity-scoring
knowledge stays cccc-scheme's own) and returns `Walk::Skip` so `walk` doesn't
also auto-descend into elements `lower_list` already visits itself, on its own
structured terms (predicate vs. `then` vs. `alternate`, scored differently).
Every other node — any `Class::Data`, or a `Code`-classified non-list —
returns `Walk::Descend`.

**A real bug in the first draft of this migration, caught by testing before
landing:** the natural-seeming `if class == Class::Data { return Walk::Skip }`
short-circuit is *wrong*. `Skip` prunes the walker from ever looking inside
that node — correct for an absolute `Quote` region, but not for a quasiquote
template, which is `Data` yet can carry a deeper `unquote` that flips its
*contents* back to `Code`. Short-circuiting on `Data` silently made every
quasiquote template opaque, undercounting any code nested inside one. The fix
is to *never* prune on `Data` — only ever `Skip` after successfully handling a
`Code`-classified `List` via `lower_list`; everything else always `Descend`,
trusting `walk`'s own depth tracking to correctly stop at true leaves.

This also fixed a **latent bug in the old hand-rolled code**, not just
matched it: `lower_quasi`'s recursion didn't track quasiquote *depth* — it
treated any `Unquote` as an immediate escape to code, so a single unquote
under a *doubly*-nested quasiquote (`` `(a `(b ,(f x))) ``) was wrongly scored
as code, when Scheme semantics (and `lispexp`'s own depth-counting `Ctx`) say
it's still data — reaching real code there needs a *second*, stacked unquote
(`` `(a `(b ,,(f x))) ``). Added
`nested_quasiquote_needs_matching_unquote_depth_to_reach_code`, which fails
under the old logic and passes under `walk`.

Re-running the full corpus audit (chibi-scheme/lem/typed-racket/Gauche) after
the migration reproduces the exact same numbers as the pre-migration 0.2.0
run — the double-unquote fix doesn't happen to be exercised by any of these
real files, so this is a from-first-principles correctness fix confirmed by a
targeted test, not something the audit corpora could have caught on their own.
