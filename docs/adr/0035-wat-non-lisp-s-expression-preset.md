# WAT is a non-Lisp S-expression preset, not a Lisp dialect

## Context

Users asked lispexp to read WebAssembly text format (WAT) files. WAT genuinely is
S-expression syntax — a module is one parenthesized tree of keywords, identifiers,
numbers, strings, and nested instruction lists — so the reader's list/atom substrate
fits it. But WAT shares almost none of the Lisp *reader surface*:

- No reader macros. `'`, `` ` ``, `#`, `|`, and `\` are WAT `idchar`s, so
  `$a'b`, `$x#y`, and `$p|q` are ordinary identifiers. Quote, quasiquote, hash
  literals, piped symbols, and `\`-escapes must all be off, or valid WAT is
  mis-tokenized. `,` and `;` are *not* idchars (they appear only in reserved
  tokens); treating `,` as unquote or `;` as a Lisp line comment would still
  mis-read valid WAT.
- Its own number grammar: decimal and `0x` hexadecimal integers with optional `_`
  separators, decimal floats with `e` exponents, hex floats with optional `.`
  fractions and `p` exponents (`0x1.8`, `0x1.8p3`), and `inf` / `nan` / `nan:0x…`.
  The published W3C text-format specification and the reference interpreter
  (`lexer.mll`, `interpreter/README.md`) accept a `p`-less hex fraction even
  though the current spectec `Thexfloat` production writes the exponent as
  mandatory; lispexp follows the reference grammar.
- `$"name with spaces"` (quoted identifiers) is one token, which the generic atom
  scanner would split at the `"`.
- Its own comments: `;;` line comments and nestable `(; … ;)` block comments. The
  block-comment opener starts with `(`, so comment recognition must win over the
  list opener.
- No keywords, character literals, booleans, dotted pairs, or `[]`/`{}` delimiters.

The crate's charter is a reader for **Lisp dialects** (ADR-0001, ADR-0029). WAT is
not a Lisp. Yet WAT is close enough in shape that consumers (formatters, indexers,
structural linters for `.wat`/`.wast`) benefit from one faithful, position-annotated
tree rather than a second parser. The alternative — telling callers to hand-assemble
an `Options` — is error-prone: the `Options` struct is `#[non_exhaustive]`, so an
external crate cannot use struct-update syntax, and it would have to discover the
dozen fields that must change (including two WAT-only semantics) by trial and error.

## Decision

Add **`Dialect::Wat`** and **`Options::wat()`** as one first-class preset, documented
as the exception to the Lisp-family framing rather than as a Lisp dialect:

1. **Its own preset, outside the families.** `Dialect::Wat` forms a single-member
   "non-Lisp S-expression" group. `Options::wat()` starts from explicit fields (not a
   Lisp base) with every reader-macro role off: `hash_syntax`, `piped_symbols`,
   `datum_labels`, `char_syntax`, `keyword_colon`, `dotted_pairs`, and the
   quote-family glyphs. `square`/`curly` are `Ordinary` so a stray bracket is part of
   an atom and never costs reader sync. Comments are `;;` line (`line_comment_double`,
   so a lone `;` is an atom and cannot swallow a same-line `)`) and nestable
   `(;`–`;)` block; the block check already runs before the `(` delimiter.
   `cr_newline` makes a lone `\r` end a line comment and be recognized by the
   unterminated-string recovery, because WAT's `newline` is `\n`, `\r`, or
   `\r\n` — line *numbering* still follows ADR-0024, so a CR-only file parses
   structurally but reports all datums as line 1.
   `(@id …)` annotations are kept as ordinary lists — the tree looks like the
   source — not dropped as WAT whitespace.

2. **WAT-specific, opt-in settings.** `Options::wat_numbers` switches atom
   classification to the WAT numeric grammar; `Options::quoted_identifiers` makes
   `$"…"` one verbatim symbol token; `Options::line_comment_double` makes a line
   comment require two copies of `line_comment` (`;;`); `Options::cr_newline`
   treats a lone `\r` as a tokenization line break. All default to `false`
   and are set only by `wat()`, so no Lisp dialect reclassifies its `1_000`,
   `inf`, or `$"x"`, or stops treating `;` as a comment. They are plain
   orthogonal fields, matching the existing `line_comment_in_atom` /
   `pipe_anon_fn` precedent (ADR-0006). An unterminated `$"…"` is
   `UnterminatedKind::QuotedIdentifier`, not `Str` (ADR-0015).

3. **Detection by extension only.** `.wat` and `.wast` map unambiguously to
   `Dialect::Wat` (`Dialect::from_extension`). No content-only signal is added:
   WAT's `(module …)` head is also Racket's, and a low-confidence guess would
   violate the "never a wrong silent default" rule (ADR-0034).

4. **Reader-only scope is unchanged.** A WAT module is the tree it looks like —
   instructions in folded or linear position are ordinary lists and symbols. The
   preset does not validate module structure, resolve indices, check types, or
   encode to binary; like every Lisp dialect it reads a superset of valid input
   (ADR-0030). `bundled_registry(Dialect::Wat)` is empty — there are no definition
   forms to annotate.

## Considered options

- **Do nothing; document a hand-built `Options`.** Rejected: the preset is the
  discoverable, correct-by-construction answer, and the two WAT fields would not
  exist for external callers to set at all.
- **Pretend WAT is a Lisp dialect with a Lisp base.** Rejected: a Lisp base enables
  quote, hash literals, and dotted pairs, each of which mis-reads valid WAT.
- **Model WAT as a separate top-level module (`wat::parse`) rather than a Dialect.**
  Rejected: `Dialect` already means "a named reader surface preset", and WAT needs
  exactly that — the same `Reader`/`Lexer`/`Options` machinery — not a second code
  path. A separate module would duplicate the entry points for no behavioural gain.
- **A content marker for extension-less WAT.** Rejected for now (see decision 3): a
  false WAT detection would mis-read Racket modules, which is worse than `None`.

## Consequences

- `Dialect::ALL`, `Display`/`FromStr`, `for_dialect`, and the detection extension
  registry gain `Wat`; `Dialect` is `#[non_exhaustive]`, so this is additive.
- WAT consumers get correct token shapes out of the box: identifiers keep their `'`,
  `#`, `|`, and `\`; numbers classify by WAT's grammar; `$"…"` is one symbol; both
  comment styles work.
- The Lisp dialects are untouched: the new `Options` fields default to `false` in
  every existing preset, and their numeric classification path is unchanged.
- The crate's README/CONTEXT now say explicitly that one non-Lisp S-expression
  surface is in scope, so the charter stays honest rather than silently drifting.
