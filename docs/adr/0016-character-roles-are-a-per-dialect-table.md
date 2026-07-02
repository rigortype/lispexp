# Reader character-role assignments are a per-dialect table, not hardcoded

Adding Guile and Janet as target dialects proved that the roles of individual punctuation characters are not universal and must be a per-dialect configuration, not baked into the lexer. **Janet** is the decisive case: its line-comment introducer is `#`, not `;` — and `;` is instead the *splice* reader macro (`;x` → `(splice x)`), while `~` is quasiquote (not unquote, as in Clojure), `,` is unquote, `@` marks mutability (`@{}` table, `@[]` array), and `|(...)` is an anonymous-function shorthand. So the line-comment character and the glyph-to-`Prefix`-role mapping become dialect-supplied tables in `Options`; we add `Prefix::Splice` and `Prefix::Mutable`, and Janet's `|` short-fn reuses `Prefix::HashFn`. **Guile** adds a smaller generalization: symbol-delimiter pairs are configurable — beyond the R7RS `|...|` it also reads `#{...}#` — so "symbol delimiters" is a set of pairs, not a single fixed form. Both dialects otherwise fit existing decisions (Janet's backtick long strings reuse ADR-0014; Guile layers on the Scheme preset with `#|…|#`/`#!…!#` block comments per ADR-0007, `#;` datum comments, and `#:` keywords).

> **Amended 2026-07-02:** the "dialect-supplied tables in `Options`" were
> originally nine separate scattered `Options` fields
> (`quote`/`quasiquote`/`unquote`/`splicing_suffix`/`deref`/`meta`/`splice`/
> `mutable`/`short_fn`). They now live in a first-class `CharRoles` sub-struct
> (`Options::roles: CharRoles`), with `CharRoles::scheme()` and
> `CharRoles::clojure()` base tables that each dialect preset overrides only
> its deltas from — matching this ADR's "a per-dialect table" framing more
> literally than nine independent toggles did.
>
> Guile's `#{...}#` extended-symbol half of the symbol-delimiter
> generalization above — described but not implemented when this ADR was
> written — is now implemented: `Options::hash_curly_symbol` (Guile-only)
> makes `#{foo bar}#` lex as one verbatim symbol token, delimited like a piped
> symbol. It is mutually exclusive with `set_literal` (both dialects that
> could plausibly want both claim the `#{` prefix), enforced by a
> `debug_assert`.
