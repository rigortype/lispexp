# sexpp

A pure-Rust reader (lexer + parser) for S-expression syntax across multiple Lisp dialects — Scheme, Guile, Racket, Common Lisp, Emacs Lisp, Clojure, Hy, Phel, Fennel, LFE, ISLisp, AutoLISP, and Janet — producing a faithful, position-annotated, code-vs-data-aware parse tree. It deliberately excludes evaluation and macro-expansion.

## Language

**Datum**:
A single parsed unit of S-expression syntax — a list, symbol, number, string, char, boolean, keyword, or a `Prefixed` reader-macro form — annotated with its source line and byte span.
_Avoid_: node, expression, form

**Dialect**:
One of the named presets of Options that bundle the individually-toggleable syntax settings sexpp can read. A Dialect is a convenience constructor, not a separate code path — the underlying Reader and Options are shared across all dialects. A Dialect may build on another's preset rather than starting from scratch (e.g. Racket layers onto the Scheme preset; Phel layers onto the Clojure preset).
_Avoid_: language

**Options**:
The orthogonal, individually-toggleable syntax settings a Reader is configured with (e.g. delimiter meaning, string/char syntax, keyword syntax, block-comment delimiters and nesting) — the mechanism Dialect support is built from, modeled after `lexpr`'s `Options` builder.
_Avoid_: config, flags (implementation detail, not the domain term)

**Delimiter meaning**:
What a bracket-like punctuation pair (`[]` or `{}`) represents in a given Dialect: an alternate list delimiter, a vector literal, a map literal, or an ordinary (non-delimiting) pair of symbol-constituent characters. Configured independently per pair via Options — e.g. Racket treats both `[]` and `{}` as `List`, Clojure/Phel treat `{}` as `Map`, ISLisp treats both as `Ordinary`.

**Lang line**:
A dialect-specific leading directive (e.g. Racket's `#lang racket`) that is not itself a Datum — it configures how the rest of the file is read. Exposed as a separate field on the parse result, not folded into the Datum tree.
_Avoid_: shebang (similar shape, but a lang line changes reader configuration, not just execution)

**Reader**:
The upper of sexpp's two layers: builds a tree of Datums on top of the Lexer. Deliberately excludes evaluation, macro-expansion, and the numeric tower.
_Avoid_: interpreter, evaluator

**Lexer**:
The lower of sexpp's two layers: turns source into a linear token stream that tiles the input, surfacing delimiters, atoms, strings, comments, and reader markers as spans. Independently consumable — a parinfer-style tool uses the Lexer without the Reader's Datum tree. Shares the same Options as the Reader.
_Avoid_: tokenizer (acceptable synonym, but "Lexer" is the canonical term here), scanner

**Reader macro**:
Reading-time syntax that tags a following Datum rather than transforming source code — e.g. quote (`'x`), quasiquote, unquote, unquote-splicing, or discard (`#_`, `#;`). Represented in the tree as a `Prefixed` datum.
_Avoid_: macro (alone — risks confusion with `defmacro`/`syntax-rules`-level code macros, which are out of scope)

**Notation**:
Whether a reader-macro form appeared in its shorthand token form (e.g. `'x`) or its explicit long-hand call form (e.g. `(quote x)`). sexpp preserves this distinction on `Prefixed` datums rather than normalizing it away, keeping future round-trip serialization feasible.

**Improper list**:
A list whose final tail is not the empty list — a dotted pair `(a . b)` or `(a b . c)`. Modeled as an ordinary List with a present dotted tail rather than a separate kind, so proper lists are the tail-absent special case.
_Avoid_: dotted list (as a distinct type — it is the same List with a tail)

**Hash literal**:
A `#`-tagged reader form treated as data — vectors (`#(...)`, `#u8(...)`), maps/structs (`#M(...)`, `#S(...)`), tagged literals (`#inst`, `#px"..."`), and dialect radix/array forms. sexpp captures the tag verbatim and does not validate it against a per-dialect whitelist.
_Avoid_: reader tag (reserve for the tag string itself)

**Datum label**:
A `#n=<datum>` definition and its `#n#` reference (Scheme/Common Lisp/Racket), marking shared or cyclic structure. sexpp records them structurally but does not resolve the graph, consistent with being reader-only.

**Form spec**:
A description of a definition form's argument structure — which position is the defined name, the arglist, the docstring, the body — derived from a macro's declared Edebug `debug` spec (leading with `&define`) plus `doc-string`/`indent` declarations. Collected into a form-spec registry (ADR-0019).

**Spec harvester**:
The component that scans Emacs Lisp source, reads each definition macro's `declare` metadata, and derives Form specs into a registry. Emacs's own definition macros are harvested and bundled as builtins. (Tentatively called "macro-collector".)

**Form annotator**:
The component that walks a Datum tree and, for each list whose head matches a Form spec, tags the children with their roles (name, arglist, docstring, body). A best-effort utility layer over the tree — it reads declared metadata, never expands macros (ADR-0019, consistent with [[reader-only-scope]]). (Tentatively called "macro-annotator".)

**Fault-tolerant parsing**:
sexpp's error-recovery model: a syntax error causes the Reader to skip to the start of the next top-level form and resume there, so a single malformed form loses only itself — never the rest of the file. Recovery resynchronizes at top-level granularity only, not within a list.

**Code vs. data classification**:
Whether a subtree should be treated as executable code (descended into for analysis) or inert data (skipped). Driven by reader-macro nesting: quote marks its contents as data; quasiquote marks its contents as data except nested unquote/unquote-splicing, which flip back to code.
