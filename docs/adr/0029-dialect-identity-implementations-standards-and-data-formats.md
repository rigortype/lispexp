# Dialect identity: presets name implementations and the *latest* standard, not standard versions; a data sub-format gets its own preset only when the language defines a restricted data reader

## Context

lispexp exposes reading through `Dialect` presets (ADR-0003/0006). As the matrix
grew — a Scheme family with several implementations and standard versions, a
Clojure family with a data notation, an Emacs Lisp with a data format — a
recurring modelling question surfaced that the per-dialect ADRs (ADR-0025 EDN,
ADR-0027 Scheme superset) each answered locally but never stated as one rule:
**what is a `Dialect`, exactly, relative to (a) a language's implementations,
(b) a language's standard *versions*, and (c) a language's data sub-formats?**
Concretely, users asked: why is there no `Dialect::R7rs`/`R5rs`? why is `EDN` a
preset but Emacs Lisp Data (`lisp-data-mode`, `.eld`) is not? why are Gauche,
Mosh, and Gambit named aliases rather than distinct presets? Answering these
consistently matters because lispexp is a **reader** (ADR-0001): the reader-level
differences between these things are often far smaller than their
language-level differences suggest.

## Decision

**1. A preset names a *reader surface*, tracked by documentation, not a pinned
standard version.** `Dialect::Scheme` / `Options::scheme()` targets the latest
*small* Scheme standard — currently **R7RS-small** — and this baseline is a
documented fact, not a type-system commitment. There is deliberately **no
per-version variant or alias** (`Dialect::R7rs`, `Dialect::R5rs`, a `"r7rs"`
`FromStr` alias): the reader-level deltas across R4RS/R5RS/R6RS/R7RS are small
(comment syntaxes `#;`/`#| |#`, piped symbols `|…|`, datum labels, `#u8(`/`#vu8(`
bytevectors, `#true`/`#false`), earlier code reads as a subset under the current
reader, and version *conformance* is a semantic concern beyond a reader. When a
future R8RS-small ships, we update this baseline in place rather than adding a
variant — a version alias would falsely imply pinning and would need churn on
every new standard.

**2. Non-conflicting implementation variants that share one reader get named
aliases, not distinct presets.** `Dialect::Gauche`, `Dialect::Mosh`, and
`Dialect::Gambit` resolve to the one shared `Options::scheme_superset()` reader
(ADR-0027), exactly as `Dialect::Phel` resolves to Clojure's reader. The alias
buys discoverability — `Dialect::from_str("gauche")`, presence in `Dialect::ALL`,
extension→dialect mapping — without pretending a per-implementation reader
exists. A distinct `Options::` constructor is added only when an implementation's
surface genuinely diverges enough to reshape the tree (Guile's `#{…}#`, Racket's
`#lang`/`[]`-lists/infix dot warrant their own presets; Gauche/Mosh/Gambit do
not).

**3. A data sub-format gets its own preset only if the language defines a
genuinely *restricted* data reader.** EDN earns `Options::edn()` (ADR-0025)
because `clojure.edn/read` is a distinct, spec-restricted reader that rejects
code-only syntax (`'`, `` ` ``, `@`, `#(`, `#'`, …); the preset both names the
intent and lets a consumer reject Clojure-only syntax in a data file. Emacs Lisp
Data (`lisp-data-mode`, `.eld`) earns **no** preset: Emacs reads code and data
with one `read`, so `'x`, `#'fn`, `#[…]`, and `#s(…)` are all valid readable data
there. `Options::emacs_lisp()` already reads `.eld` exactly. Building a
restricted "elisp-data" preset would *misrepresent* Emacs by rejecting input its
reader accepts. "Everything is data" is a semantic stance the consumer holds,
not a lexical restriction lispexp imposes (ADR-0001).

## Considered options

- **Per-standard-version presets/aliases (`r7rs`, `r5rs`, `r6rs`).** Rejected:
  the crate is reader-only and the reader deltas are tiny; the current reader
  reads older code as a subset; R6RS's one distinctive form, `#vu8(…)`, already
  reads as a verbatim `HashLiteral` (ADR-0011) and is named by the superset. A
  consumer that truly needs to *reject* a newer form toggles the relevant
  `Options` field itself.
- **A `"r7rs"` `FromStr` alias for discoverability only.** Rejected: it implies
  a version guarantee the type does not make, and becomes actively misleading
  the moment `scheme()` tracks a newer standard.
- **Distinct `Options::gauche()`/`mosh()`/`gambit()` presets.** Rejected: their
  `.scm` surface is a non-conflicting union already served by one superset
  reader (ADR-0027); three constructors returning the same `Options` add
  surface without behaviour.
- **A restricted `Options::emacs_lisp_data()` mirroring `edn()`.** Rejected: it
  has no basis in Emacs, which uses a single unrestricted `read`; it would
  reject valid `.eld` data.

## Consequences

- The `Dialect` enum stays small and honest: one variant per distinct reader
  surface, plus discoverability aliases that resolve to a shared reader. New
  Scheme standards cost a documentation edit, not an API change.
- Consumers get answers in the rustdoc: `Dialect::Scheme`/`Options::scheme()`
  state the version-tracking policy; `Dialect::EmacsLisp`/`Options::emacs_lisp()`
  state that they cover `.eld`; the Gauche/Mosh/Gambit variants point at the
  shared superset.
- Consistent with reader-only scope (ADR-0001), the Options-delta preset model
  (ADR-0003/0006), and the two prior per-dialect decisions it generalizes
  (ADR-0025, ADR-0027).
- `Dialect` is `#[non_exhaustive]`, so adding the alias variants was additive;
  the policy above does not preclude a future genuinely-distinct preset if some
  dialect's data format turns out to have a restricted reader of its own.
