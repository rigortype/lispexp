# Error recovery resynchronizes at top-level form granularity

lispexp must be fault-tolerant: a single malformed form must not lose the whole file's parse. We considered recovering within a list (e.g. salvaging the other elements of `(a b #\ c)` even though one element is broken) versus recovering only at the top level (skip to the start of the next top-level form and resume there, discarding the whole enclosing form). We chose top-level-only recovery: `cccc` measures complexity per top-level construct (functions, etc.), so losing an entire malformed top-level form has little practical cost, while within-list recovery would meaningfully complicate the parser's error-handling logic for marginal benefit.

> **Amended 2026-07-02:** the implementation now recovers more finely than
> "discard the whole enclosing form," and does so without ever losing
> subsequent input. The actual policy:
>
> - A malformed token found while scanning a list's elements is skipped in
>   place, with a diagnostic (`ErrorKind::MalformedToken`); the list keeps
>   building around it rather than being discarded wholesale.
> - An unclosed list or hash literal is retained **partially** — its
>   successfully-read items form the datum, reported with
>   `ErrorKind::UnclosedList` — rather than the whole form being thrown away.
> - A dangling prefix or discard (`'` / `#;` / `#+` / … with no following
>   datum) reports `ErrorKind::DanglingPrefix` and does not consume or lose
>   the forms that follow it; earlier drafts of the top-level loop treated a
>   dangling prefix as EOF and silently dropped the rest of the file, which is
>   now fixed.
> - An unterminated string no longer swallows every following form to EOF:
>   the lexer backtracks the unterminated token to just before the next
>   line-start `(` after the opening quote, so the reader resynchronizes and
>   recovers the following code instead of discarding it.
> - Nesting depth is capped (`MAX_DEPTH = 200`, reported as
>   `ErrorKind::DepthLimitExceeded`) so pathologically deep input can never
>   overflow the stack; `parse` is documented to never panic, and the cap is
>   what makes that true regardless of input depth. The too-deep subtree is
>   skipped as a unit and prior siblings are kept.
>
> None of this contradicts the top-level-vs-within-list granularity tradeoff
> above — recovery still resynchronizes at token/list boundaries rather than
> attempting semantic repair — but "discarding the whole enclosing form" was
> too strong a description of what actually ships.
