# Note: reader faithfulness from downstream feedback, and building real-code corpora

_2026-07-04. Context: three Phel reader fixes driven by lisplens feedback (0004–0006),
plus adding real-code corpora for the seven thin-tested dialects (Phel, ISLisp, AutoLISP,
Janet, Fennel, Hy, LFE). Written as reusable lessons for the next dialect fix / corpus add._

## 1. Verify a dialect quirk against the upstream lexer, not against the fix someone proposed

Downstream feedback (lisplens) came as Markdown with a *proposed* fix. One proposal was
subtly wrong, and only reading the real implementation caught it:

- **0005 (`|(…)` short fn).** The feedback suggested Janet's mechanism — `roles.short_fn:
  Some('|')` — which fires on `|` before *any* datum. But Phel's `Lexer.php` tokenises the
  literal two-char sequence `\|\(` (a `|` **immediately** before `(`), the delimiter-coupled
  twin of Clojure's `#(`. Janet's rule would misparse `|foo` as `HashFn(foo)` when Phel reads
  it as the ordinary symbol `|foo`. We added a dedicated `pipe_anon_fn` flag instead.

Rule of thumb: **when you implement a reader quirk, open the target implementation's
lexer/reader source and match its actual grammar.** For Phel that was the atom regex
(`[^()\[\]{},\`@ \n\r\t#]+`, which admits `;` → 0004) and the char-literal regex with its
negative lookahead `(?![A-Za-z0-9_\-\\])` (→ 0006, `\Foo\Bar` FQNs). The feedback is a
*bug report and a lead*, not a spec. Comprehensive design stays on our side (ADR-0030).

## 2. Real-code validation is the highest-signal test we have

Before/after over the phel-lang checkout (310 `.phel` files), quantified the fix far better
than any unit test:

| | before | after |
|---|---:|---:|
| structural parse errors | 2 (`UnclosedList`) | 0 |
| `\|(…)` anon fns | 0 (mis-read) | 7 |
| `\Foo` FQN symbols | 0 (read as chars) | 648 |
| char literals | 817 (781 mis-read) | 36 |

648 FQNs is the point: feedback 0006 called PHP-interop FQNs "pervasive", and the corpus
turned that claim into a number. **A throwaway parse-harness over a real corpus is worth
writing even for a single fix** (pattern: `examples/<x>_check.rs`, run, then delete before
commit — do not commit it).

## 3. Corpus triage discipline: fix the reader, or exclude with a written reason — never paper over

Every corpus failure is exactly one of:

1. **A genuine reader bug** → fix the reader (none surfaced this round — the Phel work had
   already closed the real gaps).
2. **A legitimate exclusion** → add to the `exclude` list *with a one-line rationale*, the
   way the existing entries do (lem's runtime reader-macro, gauche's `__END__`).

Kinds of legitimate exclusion we hit, worth recognising next time:

- **Intentionally-broken fixtures.** Hy's `compiler_error.hy` (unterminated string), Fennel's
  `test/bad/*` (parser-error suite). The reader flagging these is *correct*.
- **Genuinely malformed upstream source.** ISLisp `example/led.lsp` was really unbalanced
  (34 `(` vs 33 `)` — a typo in a WiringPi demo). Confirm with a paren count before blaming
  the reader.
- **Non-standard, implementation-specific syntax.** ISLisp `example/*.lsp` using a `|>`
  threading operator: ISO ISLisp `|…|` is multiple-escape (bar) symbol syntax (we model it
  deliberately — the `piped_symbol_with_spaces` unit test pins that intent), so a bare `|>`
  legitimately opens an unterminated bar symbol. Out of the reader *surface*, not a bug.

And the faithful-reader nuance that argues *against* over-excluding: Fennel's
structurally-valid-but-semantically-invalid `test/bad/` files **still parse clean**, and
should — lispexp reports structure, not semantics (ADR-0030). Keep those in.

Before changing a preset to make a corpus pass, check whether an existing unit test pins the
current behaviour as intentional (as `piped_symbol_with_spaces` did for ISLisp). If so, the
corpus file is the outlier, not the preset.

## 4. Picking a corpus: "code in language X" ≠ "an implementation of X"

`clautolisp` is a **Common Lisp implementation of AutoLISP**. Blindly globbing it would parse
216 `.lisp` (Common Lisp) files with the AutoLISP reader — nonsense. But its `.lsp` files are
genuine "Pure AutoLISP" (test harness, spec, probes). **Matching only `.lsp` gave a clean
148-file AutoLISP corpus** and excluded the CL implementation for free.

Vetting checklist before adding a corpus submodule:
1. `find … | sed 's/.*\.//' | sort | uniq -c` — see the extension mix.
2. Sample a few target-extension files — confirm they're the dialect, not the host language.
3. Parse-test the whole set with the intended `Options::…()` *before* committing; read the
   failures and decide fix-vs-exclude.
4. Pick `min_files` comfortably below the real count (hollow-green guard), and remember the
   harness already skips-and-reports non-UTF-8 files (lispexp is UTF-8 by contract, ADR-0017)
   and skips the whole corpus if the submodule isn't checked out.

## 5. Result / numbers to date

Seven new `--depth 1` submodules under `tests/corpus/`, **977 real files, all clean, zero
reader changes**: phel-lang 310, eisl 179, clautolisp 148, janet 105, fennel 91, hy 76, lfe 68.
Every thin dialect now has real-code coverage alongside its unit tests.

## 6. Process notes

- **Downstream agents opening PRs directly on us is the irregular path.** Reconcile by taking
  ownership of the comprehensive design: supersede the partial PR (expanded #19 to cover all
  three Phel gaps), and de-stack orthogonal work (#20 `keep_discarded` had been stacked on the
  Phel branch — rebased it onto master so the two merge independently).
- **Merge order for two field-adding PRs on the same struct:** merge the smaller/base one
  first, then rebase the larger onto the new master and resolve the field-ordering conflict
  once. Keep *both* new fields (don't let the resolution drop one).
- Fold rustfmt/lint fixups into their parent commit (`--amend`); force-push topic branches
  freely; keep unrelated concerns in separate PRs.
