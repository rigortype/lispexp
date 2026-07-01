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
- `VarQuote`/`FunctionQuote` (`#'foo`) → `Code` (a resolved var/function
  reference); `Deref` (`@x`), `Splice` (Janet `;x`), `ReadEval` (`#.x`),
  `HashFn` (`#(...)`) → `Code`.
- `HashLiteral` (`#(1 2 3)`, `#u8(...)`, tagged `#inst …`) → `Data`;
  `LabelRef` (`#n#`) → `Data`; `Discard` (`#_`/`#;`) → `Data`.
- `Meta` (`^m x`), `Mutable` (Janet `@{}`), `Label` inner → **context-transparent**
  (inherit the parent's class).
- `ReaderConditional`: guarded forms are context-transparent; the feature-test
  expression is `Data`.

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
