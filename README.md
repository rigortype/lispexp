# Lisp S-Expressions Parser (lispexp)

[![Crates.io](https://img.shields.io/crates/v/lispexp.svg)](https://crates.io/crates/lispexp)
[![docs.rs](https://img.shields.io/docsrs/lispexp)](https://docs.rs/lispexp/latest/lispexp/)
[![License](https://img.shields.io/github/license/rigortype/lispexp)](https://github.com/rigortype/lispexp/blob/master/LICENSE)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/rigortype/lispexp)

A pure-Rust **reader** (lexer + parser) for S-expression syntax across many Lisp-family dialects, producing a faithful, position-annotated, code-vs-data-aware parse tree.

📖 **API documentation:** <https://docs.rs/lispexp/latest/lispexp/>

lispexp is deliberately reader-only: it does **not** evaluate, expand macros, or interpret the numeric tower. It reads source text into data — the shape, positions, and reader-macro structure needed to statically analyze Lisp code.

## Features

- **One reader, many dialects.** [Scheme][] ([R<sup>7</sup>RS-small][R7RS], [Guile], [Racket], [Gauche], [Mosh], [Gambit]), [Common Lisp], [Emacs Lisp], [Clojure], [Hy], [Phel], [Fennel], [LFE (Lisp Flavoured Erlang)][LFE], [ISLisp], [AutoLISP], [Janet], and [EDN] — selected via `Options` presets built from orthogonal, individually-toggleable syntax settings.
- **Position-annotated.** Every datum carries a byte span and 1-based start line; a `LineIndex` maps offsets to 1-based (line, byte-column).
- **Code vs. data aware.** Quote/quasiquote/unquote structure is preserved, and a pruning `walk` classifies each node as `Code` or `Data` so consumers descend into code and skip quoted data.
- **Fault-tolerant.** Malformed input never panics; the reader returns a partial tree plus structured diagnostics, resynchronizing at the next top-level form, with a bounded recursion depth.
- **Zero-copy.** The parse tree borrows `&str` slices from the source; verbatim round-trip is lossless via source spans.
- **Two layers.** A tree reader (`parse`) and an independent token stream (`lex`) that tiles the input — for consumers such as a parinfer backend that need lexical state rather than a tree.
- **Definition-aware utilities.** An opt-in `annotate` module tags definition forms (name, arglist, docstring, body, method dispatch) across dialects — from a bundled per-dialect core plus a **spec harvester** that learns a project's own def-macros from the structure each dialect already exposes (an elisp `declare`/arglist, a Clojure `:arglists`/`:style/indent`, a Scheme `syntax-rules` pattern). An `indent` module harvests Emacs Lisp indent specs.
- **Pure Rust,** no `unsafe`, zero dependencies — cross-compiles cleanly. MSRV 1.70.

## Non-goals

lispexp is a faithful **reader**, not a syntax checker, validator, linter, or conformance tool. It does not evaluate, expand macros, or interpret the numeric tower (it reads code into data), and it does not certify that input is valid in any particular Lisp implementation — it accepts a *superset* of what a given implementation's reader would, reading unknown reader tags (`#foo(…)`), dialect-foreign forms (R6RS `#vu8(…)` under Scheme), and un-interpreted numbers faithfully as data (ADR-0011, ADR-0030).

It does report the **structural** problems that fall out of parsing — unbalanced/mismatched/unexpected delimiters, dangling reader-macro prefixes, malformed tokens — through `Parsed::errors` (an `ErrorKind` per issue), always on; `parsed.errors.is_empty()` is a usable "structurally clean" check. Anything dialect-*semantic* (is this tag/number/keyword legal here?) is out of scope by design; a stricter or dialect-aware validator is a thin layer a consumer builds on `errors` and `parse_form_at`, not a mode of the reader.

### A substrate for static analysis

Stopping at the syntactic layer is what makes lispexp a good *foundation* for higher-level tools — linters, indexers, formatters, complexity analyzers. lispexp supplies the **syntactic substrate**: a faithful position-annotated tree, structural diagnostics, the [code-vs-data walker](docs/adr/0026-code-vs-data-walker.md) (so a tool never lints inside quoted data), the definition-form [`annotate`](https://docs.rs/lispexp/latest/lispexp/annotate/) module (name/arglist/docstring/body/method-dispatch structure), [`indent`](https://docs.rs/lispexp/latest/lispexp/indent/) specs, and positioned reparse for editor integration. A tool adds the **semantic layer** — name binding, scope, macro knowledge, dialect rules — on top; it completes lispexp rather than fighting it (the same mechanism-vs-policy split the reader uses for write-safety). One seam worth knowing: the Datum tree drops comments and whitespace, so a trivia-sensitive tool reads those from the independent [`lex`](https://docs.rs/lispexp/latest/lispexp/fn.lex.html) token stream and correlates the two by byte span.

## Install

```sh
cargo add lispexp
```

## Usage

```rust
use lispexp::{parse, Options};

let parsed = parse("(define (square x) (* x x))", &Options::scheme());
assert!(parsed.errors.is_empty());
assert_eq!(parsed.data[0].head_symbol(), Some("define"));
assert_eq!(parsed.data[0].items().unwrap().len(), 3);
```

Pick a dialect with a preset (`Options::clojure()`, `Options::emacs_lisp()`, `Options::edn()`, …) or `Options::for_dialect(Dialect::Racket)`, then adjust individual fields by assignment. The reader is fault-tolerant, so always inspect `parsed.errors` alongside `parsed.data`.

Beyond the core reader, the crate exposes: `lex` / `Lexer` (the token layer), `walk` (a code-vs-data pruning visitor), `parse_form_at` (positioned single-form reparse for incremental validation), `LineIndex` (offset ↔ line/column), and the `annotate` and `indent` utility modules.

### Scheme support

Scheme is a family — R7RS-small plus implementations that extend its reader — and lispexp reads it through a preset per variant:

- **`Options::scheme()` — exact R7RS-small.** A strict conformance reader: the [chibi-scheme] reference implementation parses with zero errors, and a stray `#/…/` or `#[…]` in genuinely R7RS code is still reported.
- **`Options::guile()` and `Options::racket()`** layer each implementation's distinctive surface on that base — `#:foo` keywords and `#'` syntax quoting, plus Racket's `#lang` line, `[]`/`{}`-as-lists, and infix dot, and Guile's `#{…}#` extended symbols. They earn dedicated presets because that syntax can reshape the tree, so it is not safe to enable unconditionally.
- **`Options::scheme_superset()` (`Dialect::SchemeSuperset`) — the tolerant `.scm` reader.** The `.scm` extension is shared by [Gauche], Mosh (R6RS), and Gambit, whose reader extensions are *non-conflicting* widenings of R7RS. Because none of them reshapes valid R7RS, a single preset unions them all: `#[…]` char-sets and `#/…/` regexps (opaque `Str` leaves), `#"…"` interpolated strings, `#vu8(…)` bytevectors, and both leading-colon `:foo` and trailing-colon `foo:` keywords. This is why Gauche, unlike Guile and Racket, needs no bespoke preset — its surface already lives in the shared superset. `Dialect::Gauche`, `Dialect::Mosh`, and `Dialect::Gambit` are still selectable by name (including `"gauche"` etc. via `FromStr`) and all resolve to this one reader. On a full Gauche checkout the superset cuts parse errors from 288 (across 40 files) to 3 (one file, a `(exit 0)`-then-trailing-data idiom no full-file reader can model). See [ADR-0027](docs/adr/0027-scheme-superset-tolerant-reader.md).

lispexp never infers a dialect across files or models the numeric tower: pick a preset per input (e.g. by file extension) and read.

### Definition annotation

The opt-in [`annotate`](https://docs.rs/lispexp/latest/lispexp/annotate/) module answers "is this form a definition, and where are its parts?" without evaluating or expanding anything ([ADR-0019](docs/adr/0019-definition-form-annotator.md), [ADR-0020](docs/adr/0020-per-dialect-definition-registries-hybrid-ownership.md)). It is three pieces:

- a **registry** of form specs — which argument is the name, arglist, docstring, body, or method dispatch — seeded per dialect with a conservative core of the uncontested def-forms (`defun`, `defn`, `define`, `defmacro`, …);
- a **spec harvester** (`harvest_source_for`) that extends the registry with a project's *own* def-macros, read from the structure each dialect already exposes rather than a hand-written table: an Emacs Lisp `declare` spec or a def-macro's arglist parameter names (also Common Lisp, Clojure, Fennel, Janet, Hy, LFE, ISLisp), a Clojure `:arglists`/`:style/indent` metadata map, or a Scheme-family `syntax-rules`/`syntax-case`/`syntax-parse` pattern ([ADR-0031](docs/adr/0031-scheme-family-annotator-and-syntax-rules-harvest.md), [ADR-0032](docs/adr/0032-cross-family-defmacro-harvester.md));
- an **annotator** that walks the tree and tags each definition's parts, so a consumer reads `(cl-defun f (x) "doc" …)` as name/arglist/docstring/body without hard-coding `cl-defun`.

Every spec carries a confidence/provenance, and the whole layer is best-effort and reader-only: it tags what it can confidently recognize — never expanding a macro or fabricating structure — and leaves the rest alone.

## Documentation

The full API reference is on docs.rs: <https://docs.rs/lispexp/latest/lispexp/>. The design is recorded in [`docs/design.md`](docs/design.md), the domain vocabulary in [`CONTEXT.md`](CONTEXT.md), and the decisions behind it in the ADRs under [`docs/adr/`](docs/adr/).

## Copyright

```
Copyright 2026 TypedDuck - USAMI Kenta <tadsan@zonu.me>

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

[Scheme]: https://www.scheme.org/
[R7RS]: https://r7rs.org/
[chibi-scheme]: https://github.com/ashinn/chibi-scheme
[Gauche]: https://practical-scheme.net/gauche/
[Mosh]: https://github.com/higepon/mosh
[Gambit]: https://gambitscheme.org/
[Guile]: https://www.gnu.org/software/guile/
[Racket]: https://racket-lang.org/
[Common Lisp]: https://common-lisp.net/
[Emacs Lisp]: https://www.gnu.org/software/emacs/manual/html_node/elisp/index.html
[Clojure]: https://clojure.org/
[Hy]: https://hylang.org/
[Phel]: https://phel-lang.org/
[Fennel]: https://fennel-lang.org/
[LFE]: https://lfe.io/
[ISLisp]: https://www.islisp.org/
[AutoLISP]: https://help.autodesk.com/view/OARXMAC/2022/ENU/?guid=GUID-16DC15FC-5329-492E-B66A-401D49CF971F
[Janet]: https://janet-lang.org/
[EDN]: https://github.com/edn-format/edn
