# Dialect detection is an explicit, content-aware core API — the reader stays passive

## Context

Choosing which `Options`/`Dialect` to read a file with is, today, entirely the
caller's job, and every consumer re-hardcodes an extension → dialect map (the
`dialect_by_extension` example hand-rolls one; lisplens, cccc do their own).
File extensions are also *ambiguous* for Lisp — `.scm` is Gauche/Mosh/Gambit/
Guile/Chez, `.lsp` is Common Lisp or AutoLISP, `.cl`/`.lisp` shade into
non-Lisps — so extension alone is not enough; disambiguation needs content, the
way [github-linguist](https://github.com/github-linguist/linguist)'s
`heuristics.yml` disambiguates shared extensions with content patterns.

ADR-0012 says the *reader* does no auto-detection: it reads with exactly the
`Options` it was handed (a one-parse-one-Options invariant), and explicitly
leaves the door open — *"opt-in self-configuration can be layered on later
without changing this core."* The open question: is that layer lispexp's to
provide, and where?

Two facts shape the answer:

- lispexp is **better placed than a regex port**. It already models each
  dialect's reader syntax and records each dialect's conventional extensions, so
  it can disambiguate *structurally* (a `#lang` directive → Racket; a shebang
  interpreter; Guile's `#{…}#` / `define-module`; AutoLISP's `(defun c:…)` /
  `(vl-…)`) rather than by brittle regex.
- Detection yields a **reader surface (`Dialect`), never an implementation**. A
  `.scm` file does not name its processor (Gauche? Chez? Guile?); picking a
  processor/command is a separate, many-to-many concern deferred to a possible
  future `Implementation` model (see `CONTEXT.md`; ADR-0029). Detection stops at
  "which reader surface reads this."

## Decision

**Ship dialect detection as an explicit, opt-in, content-aware API in the
`lispexp` core — a `detect` module — while the reader stays passive.** This is
exactly the opt-in layer ADR-0012 anticipated: a consumer *calls* `detect` to
*choose* an `Options`; `parse` still reads only what it is handed. Nothing about
the reader's one-parse-one-Options invariant changes.

Surface:

- `Dialect::extensions(self) -> &'static [&'static str]` and
  `Dialect::from_extension(ext) -> &'static [Dialect]` — the extension registry,
  centralizing data currently scattered across the `Dialect` rustdoc. A shared
  extension returns **several** candidates, ordered by prior likelihood.
- `detect::detect(filename: Option<&str>, source: &str) -> Detection` — combines
  the extension candidates with content signals and returns a
  `Detection { dialect: Option<Dialect>, confidence, reason }`.
- `detect::detect_project(files) -> Option<Dialect>` — aggregates per-file
  detections (confidence-weighted) into one project dialect. This is an
  *explicit* call over a caller-supplied set, so — unlike a reader that silently
  infers across files — it is order-independent and does not violate ADR-0012.

**Why core, not a companion crate.** Unlike the Emacs indent data (ADR-0033:
editor-specific, bulk, version-churning → companion crate), the extension
registry is editor-*neutral*, small, stable, dependency-free language knowledge,
central to *using* the reader — the natural neighbour of `Dialect::from_str`,
which is already core. Content heuristics add no dependency (they scan text /
reuse the reader's own structural knowledge). If the heuristics ever balloon to
a linguist-scale corpus, extracting them to a `lispexp-dialects` crate is a
later, additive move; nothing about the API here forecloses it.

**Detection is best-effort, and says so.** Confidence is a first-class part of
the result (`High` — a `#lang`/shebang directive or an unambiguous extension;
`Medium` — a shared extension resolved by a content signal; `Low` — a weak
content-only guess; and `dialect: None` when nothing fires). Detection never
*fails closed* into a wrong silent default; a caller reads the confidence and
decides.

## Consequences

- Consumers stop hand-rolling extension→dialect maps and shared-extension
  disambiguation; they call one core API and get a dialect plus a confidence to
  gate on. The reader is untouched (ADR-0012 preserved).
- The registry is honest about ambiguity (candidate *lists*, not a forced single
  answer) and about scope: it resolves reader surfaces, not implementations
  (the `Implementation`/command axis stays deferred — `CONTEXT.md`).
- Content heuristics are documented as best-effort and live in one module, so
  they can grow (or be extracted to a `lispexp-dialects` companion) without
  touching the reader or the `Options`/`Dialect` model.
- ISLisp and other dialects without a distinctive extension or reliable content
  marker are simply not detected (no candidate), rather than guessed at — a
  `None`/`Low` result is the honest outcome.
