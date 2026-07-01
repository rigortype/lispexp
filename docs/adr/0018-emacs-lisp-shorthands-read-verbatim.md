# Emacs Lisp shorthands are read verbatim; prefix expansion is deferred

Emacs Lisp [shorthands](https://www.gnu.org/software/emacs/manual/html_node/elisp/Shorthands.html)
let a symbol's written prefix expand to a longer one at read time — e.g. with a
`read-symbol-shorthands` file-local variable mapping `snu-` to
`some-nice-package--`, the source `snu-foo` denotes the symbol
`some-nice-package--foo`. Two options exist for sexpp: (a) parse the
`read-symbol-shorthands` file-local variable and apply the expansion at read
time, or (b) not interpret shorthands at all — read and emit each symbol as its
verbatim lexical text, unexpanded. We choose **(b)** for now: `snu-foo` reads as
`Symbol("snu-foo")`. This matches sexpp's existing stance — the reader keeps raw
symbol slices and performs no semantic interpretation (ADR-0001), file-level
directives are captured but not acted on (ADR-0012), and file-local concerns
belong at the consumer/boundary (like encoding, ADR-0017, and case-folding). It
is also already the implemented behavior, so no expansion logic is added. Option
(a) — reading the shorthand table and expanding — is a plausible future opt-in
(e.g. behind a feature or an Options flag), deferred until a concrete need
arises, and it would require the reader to first parse a file-local-variables
block, which is out of scope today.
