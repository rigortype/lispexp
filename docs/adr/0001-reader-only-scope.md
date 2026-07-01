# sexpp is reader-only: no evaluator or macro-expander

sexpp exists to give `cccc`'s Lisp adapters a faithful, position-annotated, code-vs-data-aware parse tree for static complexity analysis, not to run Lisp code. We deliberately scope out evaluation, `syntax-rules`/`defmacro` macro-expansion, and an exact numeric tower — even though several existing S-expression crates blend reading and evaluation — because `cccc` only needs to recognize special forms by head symbol and classify code vs. data, and staying reader-only avoids the per-dialect complexity of hygienic macros and numeric towers.
