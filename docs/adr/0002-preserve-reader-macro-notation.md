# Preserve reader-macro notation (shorthand vs. longhand) instead of normalizing

Quote-family reader macros (`quote`/`quasiquote`/`unquote`/`unquote-splicing`) can be written as shorthand tokens (`'x`, `` `x ``, `,x`, `,@x`) or as their equivalent long-hand call form (`(quote x)`, ...). We could normalize both into the same `Prefixed` datum shape and discard which one was written, simplifying the tree, but instead keep both as `Prefixed` while tagging each with a `Notation::Shorthand`/`Notation::Longhand` attribute. This keeps code/data classification uniform (both notations dispatch the same way) at the cost of one small enum field, and — since `Datum` already retains source spans and the original source string, making verbatim round-trip of an unmodified parse free — this decision keeps a future constructive/round-trip serializer feasible without redesigning the tree later.

> **Amended 2026-07-02:** the original text implied longhand folding
> (recognizing `(quote x)` as the same shape as `'x`) is unconditional. It is
> not: folding is gated per dialect by `Options::fold_longhand` (on for the
> Scheme/Lisp family; off for Clojure/EDN/Janet/Hy/Fennel, whose longhand
> spellings differ or whose shorthand glyph is not reader syntax in the first
> place), further gated per family by whether the specific glyph is set on
> `Options::roles` (e.g. `quote` only folds if `CharRoles::quote` is set), and
> by `Options::fold_case_insensitive` for the case-insensitive readers (Common
> Lisp, ISLisp, AutoLISP), so `(QUOTE X)` folds there but not in
> case-sensitive Emacs Lisp. Folding also never applies inside a hash
> literal's inner (data) list — `#(quote x)` is a two-element vector, not a
> folded quote.
