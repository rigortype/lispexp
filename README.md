# Lisp S-Expressions Parser (lispexp)

A pure-Rust **reader** (lexer + parser) for S-expression syntax across many Lisp-family dialects, producing a faithful, position-annotated, code-vs-data-aware parse tree.

lispexp is deliberately reader-only: it does **not** evaluate, expand macros, or interpret the numeric tower. It reads source text into data — the shape, positions, and reader-macro structure needed to statically analyze Lisp code.

## Features

- **One reader, many dialects.** [Scheme][]([R<sup>7</sup>RS-small][R7RS]), [Guile], [Racket], [Common Lisp], [Emacs Lisp], [Clojure], [Hy], [Phel], [Fennel], [LFE (Lisp Flavoured Erlang)][LFE], [ISLisp], [AutoLISP], and [Janet] — selected via `Options` presets built from orthogonal, individually-toggleable syntax settings.
- **Position-annotated.** Every datum carries a byte span and 1-based start line.
- **Code vs. data aware.** Quote/quasiquote/unquote structure is preserved so consumers can descend into code and skip quoted data.
- **Fault-tolerant.** Malformed input never panics; the reader returns a partial tree plus diagnostics, resynchronizing at the next top-level form.
- **Zero-copy.** The parse tree borrows `&str` slices from the source; verbatim round-trip is lossless via source spans.
- **Pure Rust,** no `unsafe`, minimal dependencies — cross-compiles cleanly.

## Status

Pre-implementation. The design is recorded in [`docs/design.md`](docs/design.md), the domain vocabulary in [`CONTEXT.md`](CONTEXT.md), and the decisions behind it in [`docs/adr/`](docs/adr/). Scheme is the first dialect target.

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
