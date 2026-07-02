# Code-vs-data walker: a pruning visitor with a binary class and quasiquote-depth flip rules

## Context

Consumers such as lisplens do syntactic, code-vs-data-aware project search
(lisplens ADR-0010): descend into code, skip quoted data. lispexp already models
the distinction (quote → data; quasiquote → data except nested unquote, which
flips back to code) via `Prefixed` datums, so consumers should not reimplement
the flip logic. What was missing is a ready-made traversal that surfaces the
classification, and a decided ruling for every prefix — not just quote and
quasiquote.

## Decision

**A pruning visitor is the core primitive, not a plain iterator.** The walker
invokes a callback `visit(&Datum, Class) -> Descend | Skip` so a consumer can
prune an entire subtree (the actual search need: don't descend into quoted
data). An ergonomic pre-order `Iterator` adapter may be layered on later, but the
visitor is primary because pruning cannot be expressed by a bare iterator.

**`Class` is binary — `Code | Data`.** "Discarded" and "inert" nodes collapse to
`Data` (a code search prunes them all the same); a third class isn't worth the
complexity. The classification criterion is uniformly "can this be evaluated?"

**Prefix ruling table** (default: top-level and list items are `Code`):

- `Quote` inner → `Data` (deep); `Quasiquote` inner → `Data` with unquotes
  flipping back per depth.
- `VarQuote`/`FunctionQuote` (`#'foo`), `Deref` (`@x`), `Splice` (Janet `;x`),
  `HashFn` (`#(...)`) → **context-transparent**: code references at top level,
  but inside a quasiquote template or quote they are data like their
  surroundings (`` `(f @x) `` — `x` is template data; only an unquote flips it
  back).
- `ReadEval` (`#.x`) → `Code` **unconditionally** — `#.` is evaluated at *read*
  time, so even a hard `Quote` cannot inert it.
- `HashLiteral` (`#(1 2 3)`, `#u8(...)`, tagged `#inst …`) → `Data`;
  `LabelRef` (`#n#`) → `Data`; `Discard` (`#_`/`#;`) → `Data` (in practice the
  reader consumes discards, so this arm only serves manually built trees).
- `Meta` (`^m x`), `Mutable` (Janet `@{}`), `Label` inner → **context-transparent**
  (inherit the parent's class).
- `ReaderConditional`: the wrapped form is context-transparent. (The CL
  feature-test datum is consumed and not retained by the reader, so no ruling
  is needed for it; Clojure's `#?(:clj a)` wraps the whole list and its
  feature keys inherit the surrounding class.)

> **Amended 2026-07-02:** the original table sent `VarQuote`/`Deref`/etc. to
> `Code` unconditionally, which reset the quasiquote depth (`` `(f @x) ``
> classified `x` as code) and let `Quote` inert a `ReadEval` — both violations
> of the "can this be evaluated?" criterion. The code-reference prefixes are
> now context-transparent and `ReadEval` escapes even a hard quote. The
> feature-test ruling was also unimplementable as written (the reader drops
> the CL feature test) and is restated as above.

**Nested quasiquote uses a depth counter, with quote as an absolute barrier.**
The walker carries a quasiquote-depth along the path: `Quasiquote` `+1`,
`Unquote`/`UnquoteSplicing` `-1`; a node is `Code` iff depth `== 0` *and* not
under a hard `Quote`. `Quote` establishes an absolute `Data` region that unquote
cannot escape (quote is not quasiquote). An unmatched unquote is clamped (never
negative) and keeps the surrounding class, consistent with fault-tolerant,
reader-only structure. This classifies double-unquote (`,,c` → `Code`)
correctly, which a boolean flag cannot.

## Consequences

- Consumers get correct, ready-made code-vs-data traversal with pruning, and
  never reimplement the flip rules.
- The ruling table and depth model are public semantics consumers will depend
  on; the binary class and the "can this be evaluated?" criterion keep it
  predictable.
- Consistent with reader-only scope (ADR-0001) and the reader-macro model
  (ADR-0002/ADR-0016): the walker interprets structure already in the tree, and
  evaluates nothing.

## Addendum (0.4.0): `Region` refines `Data` for the pruning use case

The original decision above justified a binary `Class` with "'discarded' and
'inert' nodes collapse to `Data` — a code search *prunes them all the same*."
Field use (a `cccc-scheme` migration to `walk`) showed that premise is **false
for the one operation the visitor exists to enable — pruning.** The natural
idiom `if class == Class::Data { Walk::Skip }` silently drops code: a
**quasiquote template is `Data`, yet a nested `unquote` inside it is code.**
Skipping on `Data` prunes that code away. The only *correct* binary-`Class`
usage is "never `Skip` on `Data`, only `Skip` nodes you handled yourself" —
which forfeits pruning entirely for data, defeating the primitive's purpose.

The classification was never wrong; the binary *did* hide the one bit a pruner
needs. So, **additively** (the binary `Class` and `walk` are unchanged, so no
consumer breaks):

- Add `Region { Code, SealedData, PorousData }` and `walk_regions`, the callback
  variant that reports it. `Region::class()` bridges back to the binary view;
  `Region::is_prunable()` is `true` only for `SealedData`.
- **Sealed** = a hard `Quote`, a `HashLiteral`, or `Discard`: nothing inside can
  become code, so `Skip` is safe. **Porous** = a quasiquote template
  (depth > 0): inert here, but a matching nested `unquote` re-enters code, so it
  must be descended into. This is exactly the depth model above, surfaced.
- `walk` is reimplemented as a thin wrapper over `walk_regions` (one traversal).
  Its docstring no longer demonstrates `Skip`-on-`Data`; that example was the
  footgun in miniature and only worked because its data happened to be sealed.

This does not add a third *classification* — "can this be evaluated?" is still
binary. It adds a third *prunability* answer, which is the distinct question a
pruning visitor actually poses.
