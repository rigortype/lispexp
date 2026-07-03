//! Bundled standard Emacs indent-spec data (ADR-0033).
//!
//! `lispexp::indent` owns the *mechanism* — [`IndentSpec`], [`IndentTable`], and
//! `harvest_indent_specs` (a file's own `declare`/`put` specs). This module
//! ships the *standard data* Emacs carries built-in (`if` → 2, `defun` → 2,
//! `lambda` → `defun`, …), so a formatter matches a file indented by a
//! fully-loaded Emacs without re-harvesting it by hand. The indent *algorithm*
//! (`calculate-lisp-indent`) stays the consumer's — this is data only.
//!
//! ```
//! use lispexp::Dialect;
//! use lispexp::indent::{harvest_indent_specs, IndentSpec};
//! use lispexp_emacs::indent::bundled_table;
//!
//! // Start from the bundled standard specs, then layer a file's own on top.
//! let mut table = bundled_table(Dialect::EmacsLisp);
//! table.merge(harvest_indent_specs("(put 'my-macro 'lisp-indent-function 1)"));
//! assert_eq!(table.get("lambda"), Some(&IndentSpec::Defun));
//! assert_eq!(table.get("when"), Some(&IndentSpec::Number(1)));
//! assert_eq!(table.get("my-macro"), Some(&IndentSpec::Number(1)));
//! ```
//!
//! # Provenance
//!
//! Emacs is the source of truth; the table below was *harvested once* from a
//! running Emacs and is bundled as constants — it is **not** fetched at runtime
//! (this crate runs no Emacs). To regenerate it against your target Emacs, run
//! `emacs -Q --batch --load dump.el`:
//!
//! ```elisp
//! ;; dump.el
//! (require 'cl-lib)(require 'cl-macs)(require 'pcase)(require 'subr-x)
//! (require 'seq)(require 'let-alist)(require 'rx)(require 'map)(require 'gv)(require 'cl-generic)
//! (require 'cc-mode)  ; common core package; adds c-lang-defconst etc.
//! (mapatoms (lambda (s)
//!   (let ((v (and (or (fboundp s) (macrop s))
//!                 (function-get s 'lisp-indent-function 'macro))))
//!     (when (or (integerp v) (eq v 'defun))
//!       (princ (format "%s %s\n" (symbol-name s) (if (eq v 'defun) "defun" v)))))))
//! ```
//!
//! The **`'macro`** third argument to `function-get` is essential — without it,
//! macro `(declare (indent …))` specs (`cl-defun`, `pcase`, …) read as nil. The
//! set below is filtered to Rust-safe identifier names; every entry is an
//! integer or `defun` spec (no `lisp-indent-function` *function* names appear,
//! so nothing here is [`IndentSpec::Function`] or [`IndentSpec::Raw`]). The
//! `require` set — notably `cc-mode`, which adds `c-lang-defconst` — determines
//! coverage; refreshing for a new Emacs release or package set is a change to
//! this crate alone, never a `lispexp` core release. Originally harvested for
//! lisplens's Emacs Lisp formatter.
//!
//! [`IndentSpec`]: lispexp::indent::IndentSpec
//! [`IndentTable`]: lispexp::indent::IndentTable

use lispexp::indent::{IndentSpec, IndentTable};
use lispexp::Dialect;

/// The bundled standard indent-spec table for `dialect`.
///
/// Populated for [`Dialect::EmacsLisp`]; every other dialect returns an empty
/// table, since these specs are Emacs-specific and no other target dialect has
/// an equivalent standard set (ADR-0031). Layer a file's own harvested specs on
/// top with [`IndentTable::merge`].
#[must_use]
pub fn bundled_table(dialect: Dialect) -> IndentTable {
    let mut table = IndentTable::new();
    if let Dialect::EmacsLisp = dialect {
        for &(sym, n) in NUMBER_SPECS {
            table.insert(sym, IndentSpec::Number(n));
        }
        for &sym in DEFUN_SPECS {
            table.insert(sym, IndentSpec::Defun);
        }
    }
    table
}

#[rustfmt::skip]
const NUMBER_SPECS: &[(&str, u32)] = &[
    ("and-let*", 1), ("atomic-change-group", 0), ("benchmark-elapse", 0), ("benchmark-progn",
    0), ("benchmark-run", 1), ("benchmark-run-compiled", 1), ("byte-compile-maybe-guarded", 1),
    ("byte-optimize--pcase", 1), ("cal-menu-x-popup-menu", 2), ("calendar-dlet", 1),
    ("calendar-in-read-only-buffer", 1), ("catch", 1), ("cl--define-built-in-type", 2),
    ("cl-block", 1), ("cl-callf", 2), ("cl-callf2", 3), ("cl-case", 1), ("cl-defgeneric", 2),
    ("cl-define-compiler-macro", 2), ("cl-defmacro", 2), ("cl-defstruct", 1), ("cl-defsubst",
    2), ("cl-deftype", 2), ("cl-defun", 2), ("cl-destructuring-bind", 2), ("cl-do", 2),
    ("cl-do*", 2), ("cl-do-all-symbols", 1), ("cl-do-symbols", 1), ("cl-dolist", 1),
    ("cl-dotimes", 1), ("cl-ecase", 1), ("cl-etypecase", 1), ("cl-eval-when", 1), ("cl-flet",
    1), ("cl-flet*", 1), ("cl-generic-define-generalizer", 1), ("cl-iter-defun", 2),
    ("cl-labels", 1), ("cl-letf", 1), ("cl-letf*", 1), ("cl-locally", 0), ("cl-macrolet", 1),
    ("cl-multiple-value-bind", 2), ("cl-multiple-value-setq", 1), ("cl-once-only", 1),
    ("cl-progv", 2), ("cl-return-from", 1), ("cl-symbol-macrolet", 1), ("cl-the", 1),
    ("cl-typecase", 1), ("cl-with-accessors", 2), ("cl-with-gensyms", 1),
    ("combine-after-change-calls", 0), ("combine-change-calls", 2), ("comment-with-narrowing",
    2), ("condition-case", 2), ("condition-case-unless-debug", 2), ("cps--add-state", 1),
    ("custom-dirlocals-with-buffer", 0), ("debugger-env-macro", 0), ("def-edebug-elem-spec",
    1), ("def-edebug-spec", 1), ("defadvice", 2), ("define-advice", 2), ("define-generic-mode",
    1), ("define-ibuffer-filter", 2), ("define-ibuffer-op", 2), ("define-ibuffer-sorter", 1),
    ("define-icon", 2), ("defmacro", 2), ("defsubst", 2), ("deftheme", 1), ("defun", 2),
    ("defvar-keymap", 1), ("delay-mode-hooks", 0), ("dlet", 1), ("dolist", 1),
    ("dolist-with-progress-reporter", 2), ("dont-compile", 0), ("dotimes", 1),
    ("dotimes-with-progress-reporter", 2), ("easy-mmode-define-navigation", 5),
    ("easy-mmode-defmap", 1), ("easy-mmode-defsyntax", 1),
    ("eldoc--documentation-strategy-defcustom", 2), ("ert-deftest", 2),
    ("ert-font-lock-deftest", 1), ("ert-font-lock-deftest-file", 1), ("ert-info", 1),
    ("ert-with-buffer-renamed", 1), ("ert-with-buffer-selected", 1),
    ("ert-with-message-capture", 1), ("ert-with-temp-file", 1),
    ("ert-with-test-buffer-selected", 1), ("eval-after-load", 1), ("eval-and-compile", 0),
    ("eval-when-compile", 0), ("flymake--with-backend-state", 2), ("gv-define-expander", 1),
    ("gv-define-setter", 2), ("gv-letplace", 2), ("handler-bind", 1), ("ibuffer-aif", 2),
    ("ibuffer-awhen", 1), ("ibuffer-save-marks", 0), ("if", 2), ("if-let", 2), ("if-let*", 2),
    ("ignore-error", 1), ("ignore-errors", 0), ("inhibit-auto-revert", 0), ("inline", 0),
    ("inline--leteval", 1), ("inline-letevals", 1), ("iter-do", 1), ("let", 1), ("let*", 1),
    ("let-alist", 1), ("let-when-compile", 1), ("letrec", 1), ("macroexp--accumulate", 1),
    ("macroexp--with-extended-form-stack", 1), ("macroexp-let2", 3), ("macroexp-let2*", 2),
    ("macroexp-preserve-posification", 1), ("map-let", 2), ("minibuffer-with-setup-hook", 1),
    ("named-let", 2), ("oclosure--lambda", 3), ("oclosure-define", 1), ("oclosure-lambda", 2),
    ("org-add-props", 2), ("org-agenda-with-point-at-orig-entry", 1),
    ("org-babel-comint-async-delete-dangling-and-eval", 1), ("org-babel-comint-in-buffer", 1),
    ("org-babel-comint-with-output", 1), ("org-babel-map-call-lines", 1),
    ("org-babel-map-inline-src-blocks", 1), ("org-babel-map-src-blocks", 1),
    ("org-babel-result-cond", 1), ("org-babel-with-temp-filebuffer", 1), ("org-cite-emphasize",
    1), ("org-cite-register-processor", 1), ("org-combine-change-calls", 2), ("org-dlet", 1),
    ("org-element-adopt", 1), ("org-element-adopt-elements", 1), ("org-element-ast-map", 2),
    ("org-element-lineage-map", 2), ("org-element-map", 2), ("org-element-with-disabled-cache",
    0), ("org-eval-in-environment", 1), ("org-export-to-buffer", 2), ("org-export-to-file", 2),
    ("org-fold-core-cycle-over-indirect-buffers", 0), ("org-fold-core-ignore-modifications",
    0), ("org-fold-core-save-visibility", 1), ("org-fold-core-suppress-folding-fix", 0),
    ("org-fold-save-outline-visibility", 1), ("org-lint-add-checker", 1), ("org-no-warnings",
    0), ("org-save-outline-visibility", 1), ("org-unbracket-string", 2),
    ("org-with-base-buffer", 1), ("org-with-gensyms", 1), ("org-with-point-at", 1),
    ("org-with-remote-undo", 1), ("org-with-syntax-table", 1), ("org-with-undo-amalgamate", 0),
    ("org-without-partial-completion", 0), ("pcase", 1), ("pcase-defmacro", 2),
    ("pcase-dolist", 1), ("pcase-exhaustive", 1), ("pcase-let", 1), ("pcase-let*", 1),
    ("prog1", 1), ("prog2", 2), ("progn", 0), ("replace--push-stack", 0), ("report-errors", 1),
    ("rx-let", 1), ("rx-let-eval", 1), ("save-current-buffer", 0), ("save-excursion", 0),
    ("save-mark-and-excursion", 0), ("save-match-data", 0), ("save-restriction", 0),
    ("save-selected-window", 0), ("save-window-excursion", 0), ("seq-doseq", 1), ("seq-let",
    2), ("static-if", 2), ("static-unless", 1), ("static-when", 1), ("thread-first", 0),
    ("thread-last", 0), ("track-mouse", 0), ("treesit--some", 1), ("treesit-node-get", 1),
    ("treesit-query-first-valid", 1), ("treesit-query-with-fallback", 1), ("unless", 1),
    ("unwind-protect", 1), ("when", 1), ("when-let", 1), ("when-let*", 1), ("while", 1),
    ("while-let", 1), ("while-no-input", 0), ("with-auto-compression-mode", 0),
    ("with-buffer-unmodified-if-unchanged", 0), ("with-case-table", 1), ("with-category-table",
    1), ("with-coding-priority", 1), ("with-connection-local-application-variables", 1),
    ("with-connection-local-variables", 0), ("with-current-buffer", 1),
    ("with-current-buffer-window", 3), ("with-decoded-time-value", 1), ("with-delayed-message",
    1), ("with-demoted-errors", 1), ("with-displayed-buffer-window", 3),
    ("with-environment-variables", 1), ("with-eval-after-load", 1), ("with-existing-directory",
    0), ("with-file-modes", 1), ("with-help-window", 1), ("with-local-quit", 0),
    ("with-locale-environment", 1), ("with-memoization", 1),
    ("with-minibuffer-completions-window", 0), ("with-minibuffer-selected-window", 0),
    ("with-mutex", 1), ("with-no-warnings", 0), ("with-output-to-string", 0),
    ("with-output-to-temp-buffer", 1), ("with-restriction", 2), ("with-selected-frame", 1),
    ("with-selected-window", 1), ("with-silent-modifications", 0), ("with-slots", 2),
    ("with-sqlite-transaction", 1), ("with-suppressed-warnings", 1), ("with-syntax-table", 1),
    ("with-system-sleep-block", 1), ("with-temp-buffer", 0), ("with-temp-buffer-window", 3),
    ("with-temp-file", 1), ("with-temp-message", 1), ("with-timeout", 1),
    ("with-undo-amalgamate", 0), ("with-window-non-dedicated", 1), ("with-work-buffer", 0),
    ("with-wrapper-hook", 2), ("without-remote-files", 0), ("without-restriction", 0),
    // cc-mode and other common core packages, captured with `(require 'cc-mode)`
    // added to the dump (see the Provenance section). `c-lang-defconst` is what
    // php-mode uses heavily.
    ("c-define-abbrev-table", 1), ("c-font-lock-doc-comments", 2),
    ("c-fontify-types-and-refs", 1), ("c-lang-defconst", 1),
    ("c-let*-maybe-max-specpdl-size", 1), ("c-make-keywords-re", 1), ("c-safe", 0),
    ("c-save-buffer-state", 1), ("c-tentative-buffer-changes", 0), ("c-with-syntax-table", 1),
    ("cc-eval-when-compile", 0), ("org-compatible-face", 1),
    ("org-fold-core-ignore-fragility-checks", 0), ("org-persist-collection-let", 1),
];

#[rustfmt::skip]
const DEFUN_SPECS: &[&str] = &[
    "autoload", "cl-defmethod", "cl-generic-define-context-rewriter", "defalias",
    "defcalcmodevar", "defclass", "defconst", "defcustom", "defface", "defgroup", "defimage",
    "define-abbrev", "define-abbrev-table", "define-alternatives", "define-auto-insert",
    "define-button-type", "define-category", "define-ccl-program", "define-char-code-property",
    "define-charset", "define-charset-internal", "define-coding-system",
    "define-compilation-mode", "define-completion-category", "define-derived-mode",
    "define-fringe-bitmap", "define-global-minor-mode", "define-globalized-minor-mode",
    "define-ibuffer-column", "define-inline", "define-key-after", "define-keymap",
    "define-mail-user-agent", "define-minor-mode", "define-multisession-variable",
    "define-obsolete-function-alias", "define-obsolete-variable-alias",
    "define-short-documentation-group", "define-skeleton", "define-translation-hash-table",
    "define-translation-table", "define-treesit-generic-mode", "define-widget",
    "define-widget-keywords", "defmath", "defvar", "defvar-local", "defvaralias",
    "easy-menu-define", "easy-mmode-define-global-mode", "easy-mmode-define-minor-mode",
    "isearch-define-mode-toggle", "iter-defun", "keymap-set-after", "lambda",
    "org-agenda--insert-overriding-header", "org-defvaralias", "pcase-lambda", "rx-define",
    "transient-append-suffix", "transient-inline-group", "transient-insert-suffix",
    "transient-remove-suffix", "transient-replace-suffix", "use-package",
    "use-package-only-one", "which-key-add-keymap-based-replacements",
    "which-key-add-major-mode-key-based-replacements",
    // cc-mode / use-package, captured with `(require 'cc-mode)` in the dump.
    "defcustom-c-stylevar", "use-package-as-one",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_specs_are_present() {
        let t = bundled_table(Dialect::EmacsLisp);
        // Number specs (note: `defun` itself harvests as an integer 2, not the
        // `defun` symbol — the data is Emacs's, verbatim).
        assert_eq!(t.get("defun"), Some(&IndentSpec::Number(2)));
        assert_eq!(t.get("defmacro"), Some(&IndentSpec::Number(2)));
        assert_eq!(t.get("if"), Some(&IndentSpec::Number(2)));
        assert_eq!(t.get("when"), Some(&IndentSpec::Number(1)));
        assert_eq!(t.get("progn"), Some(&IndentSpec::Number(0)));
        // `defun`-spec forms.
        assert_eq!(t.get("lambda"), Some(&IndentSpec::Defun));
        assert_eq!(t.get("defclass"), Some(&IndentSpec::Defun));
        assert_eq!(t.get("define-minor-mode"), Some(&IndentSpec::Defun));
        // cc-mode coverage (the reason for `(require 'cc-mode)` in the dump).
        assert_eq!(t.get("c-lang-defconst"), Some(&IndentSpec::Number(1)));
    }

    #[test]
    fn covers_both_spec_kinds_and_is_nonempty() {
        let t = bundled_table(Dialect::EmacsLisp);
        assert_eq!(t.len(), NUMBER_SPECS.len() + DEFUN_SPECS.len());
        assert!(t.iter().any(|(_, s)| matches!(s, IndentSpec::Number(_))));
        assert!(t.iter().any(|(_, s)| *s == IndentSpec::Defun));
    }

    #[test]
    fn data_has_no_duplicate_symbols() {
        // NUMBER_SPECS and DEFUN_SPECS must not overlap or self-repeat, or the
        // table len assertion above would silently under-count.
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for &(sym, _) in NUMBER_SPECS {
            assert!(seen.insert(sym), "duplicate symbol: {sym}");
        }
        for &sym in DEFUN_SPECS {
            assert!(seen.insert(sym), "duplicate symbol: {sym}");
        }
    }

    #[test]
    fn non_emacs_dialects_get_an_empty_table() {
        assert!(bundled_table(Dialect::Scheme).is_empty());
        assert!(bundled_table(Dialect::CommonLisp).is_empty());
        assert!(bundled_table(Dialect::Clojure).is_empty());
    }
}
