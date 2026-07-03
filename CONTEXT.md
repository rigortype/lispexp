# lispexp

A pure-Rust reader (lexer + parser) for S-expression syntax across multiple Lisp dialects — Scheme, Guile, Racket, Common Lisp, Emacs Lisp, Clojure, Hy, Phel, Fennel, LFE, ISLisp, AutoLISP, and Janet — plus a tolerant `.scm` "Scheme superset" (ADR-0027) that reads Gauche/Mosh/Gambit reader extensions, producing a faithful, position-annotated, code-vs-data-aware parse tree. It deliberately excludes evaluation and macro-expansion.

## Language

**Datum**:
A single parsed unit of S-expression syntax — a list, symbol, number, string, char, boolean, keyword, or a `Prefixed` reader-macro form — annotated with its source line and byte span.
_Avoid_: node, expression, form

**Dialect**:
One of the named presets of Options that bundle the individually-toggleable syntax settings lispexp can read. A Dialect is a convenience constructor, not a separate code path — the underlying Reader and Options are shared across all dialects. A Dialect may build on another's preset rather than starting from scratch (e.g. Racket layers onto the Scheme preset; Phel layers onto the Clojure preset). Dialects group into **families** that share a base reader — the Scheme family (Scheme, Guile, Racket, Gauche, Mosh, Gambit, Scheme superset) and the Clojure family (Clojure, Phel, EDN). A Dialect names a **reader surface**, not a pinned standard version: `Scheme` tracks the latest *small* Scheme standard (currently R7RS-small) and reads earlier RnRS as a subset, so lispexp adds no per-version Dialect (ADR-0029). Some Dialects are **aliases** that resolve to a shared reader rather than a distinct preset — Gauche, Mosh, and Gambit all select the Scheme superset — a named entry point, not a separate code path.
_Avoid_: language

EDN is a Dialect too, but a **data-only** one: its preset layers on Clojure with the code-only reader syntax (`#(`, `#'`, `#?`, `#"…"`, `@`, and the quote family `'`/`` ` ``/`~`/`^`) turned off, since EDN's `clojure.edn` reader is a genuinely *restricted* data reader (ADR-0025). A data format earns its own preset only when the language defines such a restricted reader. Emacs Lisp Data (`lisp-data-mode`, `.eld`) does *not*: Emacs reads code and data with one reader, so `.eld` reuses the Emacs Lisp preset unchanged — "everything is data" there is a semantic stance, not a lexical restriction (ADR-0029).

**Scheme superset**:
The tolerant `.scm` reader (`Options::scheme_superset`, `Dialect::SchemeSuperset`) that widens R7RS-small with the non-conflicting reader extensions shared by the `.scm`-using implementations — Gauche `#[…]` char-sets and `#/…/` regexps (opaque string leaves), `#"…"` interpolated strings, `#vu8(…)` bytevectors, and both leading-colon `:foo` and trailing-colon `foo:` keywords. A strict *widening*: it only affects input strict `Scheme` would reject or mis-split, never reclassifying valid R7RS. Gauche, Mosh, and Gambit are aliases for it (ADR-0027, ADR-0029).
_Avoid_: dialect inference (lispexp never detects a dialect across files; the superset is one per-file reader)

**Options**:
The orthogonal, individually-toggleable syntax settings a Reader is configured with (e.g. delimiter meaning, string/char syntax, keyword syntax, block-comment delimiters and nesting) — the mechanism Dialect support is built from, modeled after `lexpr`'s `Options` builder.
_Avoid_: config, flags (implementation detail, not the domain term)

**Delimiter meaning**:
What a bracket-like punctuation pair (`[]` or `{}`) represents in a given Dialect: an alternate list delimiter, a vector literal, a map literal, or an ordinary (non-delimiting) pair of symbol-constituent characters. Configured independently per pair via Options — e.g. Racket treats both `[]` and `{}` as `List`, Clojure/Phel treat `{}` as `Map`, ISLisp treats both as `Ordinary`.

**Lang line**:
A dialect-specific leading directive (e.g. Racket's `#lang racket`) that is not itself a Datum — it configures how the rest of the file is read. Exposed as a separate field on the parse result, not folded into the Datum tree.
_Avoid_: shebang (similar shape, but a lang line changes reader configuration, not just execution)

**Reader**:
The upper of lispexp's two layers: builds a tree of Datums on top of the Lexer. Deliberately excludes evaluation, macro-expansion, and the numeric tower.
_Avoid_: interpreter, evaluator

**Lexer**:
The lower of lispexp's two layers: turns source into a linear token stream that tiles the input, surfacing delimiters, atoms, strings, comments, and reader markers as spans. Independently consumable — a parinfer-style tool uses the Lexer without the Reader's Datum tree. Shares the same Options as the Reader.
_Avoid_: tokenizer (acceptable synonym, but "Lexer" is the canonical term here), scanner

**Reader macro**:
Reading-time syntax that tags a following Datum rather than transforming source code — e.g. quote (`'x`), quasiquote, unquote, unquote-splicing, or discard (`#_`, `#;`). Represented in the tree as a `Prefixed` datum.
_Avoid_: macro (alone — risks confusion with `defmacro`/`syntax-rules`-level code macros, which are out of scope)

**Notation**:
Whether a reader-macro form appeared in its shorthand token form (e.g. `'x`) or its explicit long-hand call form (e.g. `(quote x)`). lispexp preserves this distinction on `Prefixed` datums rather than normalizing it away, keeping future round-trip serialization feasible.

**Improper list**:
A list whose final tail is not the empty list — a dotted pair `(a . b)` or `(a b . c)`. Modeled as an ordinary List with a present dotted tail rather than a separate kind, so proper lists are the tail-absent special case. The `.` separator's own byte span is recorded alongside the tail (`dot: Option<Span>`, surfaced as `Datum::dot_span()`), so a text-based reindenter can align a tail continuation under the dot without re-scanning the source; `dot` is `Some` iff `tail` is `Some`.
_Avoid_: dotted list (as a distinct type — it is the same List with a tail)

**Hash literal**:
A `#`-tagged reader form treated as data — vectors (`#(...)`, `#u8(...)`), maps/structs (`#M(...)`, `#S(...)`), tagged literals (`#inst`, `#px"..."`), and dialect radix/array forms. lispexp captures the tag verbatim and does not validate it against a per-dialect whitelist.
_Avoid_: reader tag (reserve for the tag string itself)

**Datum label**:
A `#n=<datum>` definition and its `#n#` reference (Scheme/Common Lisp/Racket), marking shared or cyclic structure. lispexp records them structurally but does not resolve the graph, consistent with being reader-only.

**Form spec**:
A description of a definition form's argument structure — which position is the defined name, the arglist, the docstring, the body — derived from a macro's declared Edebug `debug` spec (leading with `&define`) plus `doc-string`/`indent` declarations. Collected into a form-spec registry (ADR-0019).

**Spec harvester**:
The component that scans source and derives Form specs into a registry from several heuristic signals via `harvest_source_for`. The portable, highest-yield signal — a def-macro's own arglist parameter names (`name`/`arglist`/`docstring`/`body`) — works across every arglist-style macro Lisp (Emacs Lisp, Common Lisp, Clojure, Fennel, Janet, Hy, LFE, ISLisp), driven by a per-dialect **harvest profile** (macro-defining heads, docstring policy) (ADR-0032). Some dialects add a higher-confidence refinement: Emacs Lisp's `declare` metadata (`debug (&define …)`, `doc-string`, `indent`) and Clojure's `:arglists`/`:style/indent` metadata (a `^{…}` map on the name or an attr-map) override the parameter-name guess with `Declared` provenance (`:arglists` names roles; `:style/indent` names only the body boundary, elisp `indent`-style). The Scheme family is harvested differently — its macros are `syntax-rules` *patterns*, not an arglist, so a separate pattern harvester reads each rule's input pattern (ADR-0031). Emacs's own definition macros are harvested and bundled as builtins. (Tentatively called "macro-collector".)

**Form annotator**:
The component that walks a Datum tree and, for each list whose head matches a Form spec, tags the children with their roles (name, arglist, docstring, body). A best-effort utility layer over the tree — it reads declared metadata, never expands macros (ADR-0019, consistent with [[reader-only-scope]]). Its tree descent follows the [[code-data-walker]] (`code_nodes`), so it annotates a definition reachable in *code* position — including one guarded by a reader/feature conditional (`#+sbcl (defun …)`) or unquoted inside a quasiquote — while skipping [[data-prunability]] sealed data like a quoted `'(defun …)` or a quasiquote template. (Tentatively called "macro-annotator".)

**Form-spec registry**:
The per-Dialect collection of Form specs the annotator matches against. lispexp bundles a conservative, high-confidence *core* per dialect (the uncontested def-forms) and exposes a builder so consumers extend or override it; the long tail of project-local or contested def-forms is the consumer's to supply (ADR-0020).

**Kind**:
A definition's raw head symbol, kept verbatim (`"defun"`, `"defn"`, `"defmethod"`). Always faithful and reader-only.
_Avoid_: type, category (reserve "category" for the optional hint below)

**Category**:
An optional, normalized classification hint on a Form spec (function / macro / variable / class / method …), attached only where the mapping is uncontested. Ambiguous forms (e.g. Clojure `def`) carry no category and expose only their Kind (ADR-0020).

**Qualifier**:
An optional method modifier appearing between a method's name and its arglist — CL/elisp `:around`, `:before`, `:after`, or user-defined. Modeled as a greedy, variable-length `Qualifiers` role that consumes children up to the first delimited list (the arglist boundary), read as tokens only (ADR-0021).

**Dispatch value**:
Clojure `defmethod`'s single arbitrary dispatch datum (e.g. `:circle` in `(defmethod area :circle …)`) — distinct from a Qualifier and modeled as its own one-Datum role (ADR-0021).

**Specializer**:
The per-parameter type/eql token in a method's *specialized arglist* — `integer` in `(x integer)`, or a whole `(eql form)`. Exposed as verbatim Datums via a `SpecializedArglist` role that splits each required parameter into a `(variable, specializer)` pair; lispexp never resolves types or evaluates the `eql` form (ADR-0021).

**Indent spec**:
Per-symbol indentation metadata harvested from Emacs's `(declare (indent …))` / `lisp-indent-function`, held in a first-class `symbol → IndentSpec` table independent of the Form-spec registry (control/binding macros like `when`/`dolist` carry indent specs but are not definitions). Typed as `Number` / `Defun` / `Function(name)` / `Raw`, with the function case holding a name only. Harvesting is Emacs-Lisp-specific for now (ADR-0022). The core crate ships the *mechanism* (types + harvester) but no standard data; the [[bundled indent data]] lives in a companion crate.

**Bundled indent data**:
The standard Emacs indent-spec table Emacs itself carries built-in (`if` → 2, `defun` → 2, `lambda` → `defun`, …), harvested from a running Emacs. It lives in the companion crate `lispexp-emacs-indent` (`bundled_table(Dialect)`), not the reader core — the data is editor-and-version-specific and regenerated by a recipe, unlike the dialect-neutral core (ADR-0033). Boundary: lispexp owns the [[indent spec]] mechanism + this data; a consumer owns the indent *algorithm* (`calculate-lisp-indent`), which is rendering, outside [[reader-only-scope]].

**Fault-tolerant parsing**:
lispexp's error-recovery model: a syntax error causes the Reader to skip to the start of the next top-level form and resume there, so a single malformed form loses only itself — never the rest of the file. Recovery resynchronizes at top-level granularity only, not within a list.

**Error kind**:
The structured classification of a parse diagnostic — a `#[non_exhaustive]` enum (unclosed list, mismatched/unexpected delimiter, malformed token, dangling prefix/tag/label, …) that replaces the old free-form message string. Variants may carry non-positional payload (e.g. expected/found delimiter) but never a Span-derived value, keeping a kind stable across the position shifts an edit causes. The human message is rendered via `Display` (ADR-0023).

**Positioned reparse**:
Reading exactly one top-level form at/after a given byte offset (`parse_form_at`), returning the form, its errors, and the end offset — spans absolute into the original source. The mechanism consumers use for cheap validate-then-write; the reader supplies it, but the "reject only newly-introduced errors" policy stays with the consumer (ADR-0023, [[fault-tolerant-parsing]]).

**Line index**:
A public `LineIndex` over a `&str` (computed once, independent of the Datum tree) mapping byte offset ↔ (1-based line, 1-based **byte** column) and line number → byte range. Columns are byte offsets; char/UTF-16 columns are the consumer's to derive from `line_range` (ADR-0024, [[reader-only-scope]] via ADR-0017). Line breaks are `\n`/`\r\n` only.

**Code vs. data classification**:
Whether a subtree should be treated as executable code (descended into for analysis) or inert data (skipped) — a binary `Code`/`Data` class assigned by the criterion "can this be evaluated?". Driven by reader-macro nesting: quote marks its contents as data; quasiquote marks its contents as data except nested unquote/unquote-splicing, which flip back to code. Modeled with a quasiquote-depth counter (quasiquote +1, unquote −1; code iff depth 0), with quote as an absolute data barrier unquote cannot escape (ADR-0026). Surfaced via the [[code-data-walker]]. For *pruning*, the binary `Data` is refined into [[data-prunability]].

**Data prunability (sealed vs. porous)**:
Whether a `Data` subtree is safe to prune with `Skip`. **Sealed** data (a hard `quote`, a hash literal, discarded content) can never contain code, so skipping it is a safe optimization. **Porous** data (a quasiquote template, depth > 0) is inert at that depth but a matching nested unquote re-enters code, so it must be descended into — skipping it silently drops that code. The classification criterion is unchanged (still "can this be evaluated?"); prunability is a distinct question the binary class did not answer, exposed as `Region { Code, SealedData, PorousData }` via `walk_regions` (ADR-0026 addendum, 0.4.0).

**Code-data walker**:
The pruning visitor lispexp exposes over a `Parsed` tree: `visit(&Datum, Class) -> Descend | Skip`, so a consumer descends into code and prunes quoted-data subtrees without reimplementing the flip logic. A best-effort utility layer over the tree; evaluates nothing (ADR-0026, [[reader-only-scope]]). The `walk_regions` variant reports [[data-prunability]] so a `Skip` never drops quasiquoted code. The visitor is primary because *arbitrary* pruning can't be a bare iterator; the fixed "walk every code node" policy is exposed as `code_nodes`, a pre-order `Iterator` that prunes sealed data and descends porous templates (both route their descent through one internal `children()` helper, so they can't diverge).
