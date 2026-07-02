# `#lang` and dialect selection are passive; the reader never self-reconfigures

The caller supplies an `Options` (typically via a `Dialect` preset); the reader never changes its own configuration mid-parse. Racket's `#lang <name>` header is captured verbatim into `Parsed.lang_line` but does **not** cause the reader to switch to a Racket configuration on the fly — mapping the open-ended space of `#lang` names (racket, racket/base, typed/racket, custom langs) to Options is a registry the reader deliberately does not own, and `cccc`'s adapters already know the dialect from file context. Likewise, the reader does **no** dialect auto-detection from file extension or content; detection, if wanted, belongs in the consumer. This preserves a one-parse-one-Options invariant that keeps the reader mechanical and predictable; opt-in self-configuration can be layered on later without changing this core.

> **Amended 2026-07-02:** "captured verbatim" overstated what
> `Parsed.lang_line` actually holds. The field holds the **trimmed language
> spec** — the leading `#lang` token and surrounding whitespace stripped, so
> `#lang racket` yields `"racket"`, not the literal line text. Decision: keep
> the trimmed form, since it is what every realistic consumer wants (a
> dialect-name string to look up, not a line to re-parse); a consumer that
> needs the untrimmed source still has it, because the `LangLine` token's span
> (available from `lex`) covers the whole verbatim line and can be sliced from
> the source directly. `Parsed.lang_line` itself is not a span and does not
> carry byte positions.
