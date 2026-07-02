# Lisp S-Expressions Parser (lispexp)

A pure-Rust **reader** (lexer + parser) for S-expression syntax across many Lisp-family dialects, producing a faithful, position-annotated, code-vs-data-aware parse tree.

lispexp is deliberately reader-only: it does **not** evaluate, expand macros, or interpret the numeric tower. It reads source text into data — the shape, positions, and reader-macro structure needed to statically analyze Lisp code.

## Features

- **One reader, many dialects.** [Scheme][] ([R<sup>7</sup>RS-small][R7RS]), [Guile], [Racket], [Common Lisp], [Emacs Lisp], [Clojure], [Hy], [Phel], [Fennel], [LFE (Lisp Flavoured Erlang)][LFE], [ISLisp], [AutoLISP], [Janet], and [EDN] — plus a tolerant `.scm` "Scheme superset" — selected via `Options` presets built from orthogonal, individually-toggleable syntax settings.
- **Position-annotated.** Every datum carries a byte span and 1-based start line; a `LineIndex` maps offsets to 1-based (line, byte-column).
- **Code vs. data aware.** Quote/quasiquote/unquote structure is preserved, and a pruning `walk` classifies each node as `Code` or `Data` so consumers descend into code and skip quoted data.
- **Fault-tolerant.** Malformed input never panics; the reader returns a partial tree plus structured diagnostics, resynchronizing at the next top-level form, with a bounded recursion depth.
- **Zero-copy.** The parse tree borrows `&str` slices from the source; verbatim round-trip is lossless via source spans.
- **Two layers.** A tree reader (`parse`) and an independent token stream (`lex`) that tiles the input — for consumers such as a parinfer backend that need lexical state rather than a tree.
- **Definition-aware utilities.** An opt-in `annotate` module tags definition forms (name, arglist, docstring, body, method dispatch) across dialects, and an `indent` module harvests Emacs Lisp indent specs.
- **Pure Rust,** no `unsafe`, zero dependencies — cross-compiles cleanly. MSRV 1.70.

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

## Documentation

API docs are on [docs.rs](https://docs.rs/lispexp). The design is recorded in [`docs/design.md`](docs/design.md), the domain vocabulary in [`CONTEXT.md`](CONTEXT.md), and the decisions behind it in the ADRs under [`docs/adr/`](docs/adr/).

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
