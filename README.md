# sexpp — renamed to `lispexp`

> [!IMPORTANT]
> **This crate has been renamed to [`lispexp`](https://crates.io/crates/lispexp).**
> `sexpp` is deprecated and receives no further updates. Please migrate — the API is identical:
>
> - `Cargo.toml`: replace `sexpp = "0.1"` with `lispexp = "0.1"`
> - Source: replace `use sexpp::…` with `use lispexp::…`
>
> Development continues at <https://github.com/rigortype/lispexp>.

A pure-Rust **reader** (lexer + parser) for S-expression syntax across many Lisp-family dialects, producing a faithful, position-annotated, code-vs-data-aware parse tree — now published as [`lispexp`](https://crates.io/crates/lispexp).

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
