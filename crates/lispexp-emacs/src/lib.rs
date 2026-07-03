//! Emacs-specific integration data and interpreters for [`lispexp`] (ADR-0033).
//!
//! `lispexp` is a dialect-neutral S-expression *reader*. This companion crate
//! is the home for **Emacs-specific** knowledge that builds on it — data and
//! interpreters that are tied to one editor (Emacs), version-sensitive, and
//! kept out of the neutral core. It depends on `lispexp`, never the reverse.
//!
//! Emacs is not an arbitrary target: Lisp tooling is historically inseparable
//! from Emacs, and the de-facto standard for modern Lisp formatting *is* the
//! indentation engine of Emacs's major modes. This crate is where a Lisp
//! formatter / linter / LSP finds the Emacs knowledge it would otherwise
//! re-derive.
//!
//! Scope, by axis (ADR-0033):
//! - **In scope:** Emacs-specific integration —
//!   - [`indent`] — the bundled standard indent-spec table.
//!   - [`local_vars`] — Emacs file-local variables: the leading `-*- … -*-`
//!     header cookie and the trailing `Local Variables:` block.
//!   - *planned* `dir_locals` — a simple evaluator for `.dir-locals.el` (an
//!     elisp data file `lispexp` already reads).
//! - **Out of scope, editor-*neutral*:** file-extension → dialect selection —
//!   that is deliberately the caller's, not Emacs-specific (lispexp ADR-0012,
//!   ADR-0034).
//! - **Out of scope, *foreign format*:** EditorConfig and the like — not
//!   S-expressions; a consumer's rendering-policy concern.
//!
//! **Read & interpret, never execute.** `.dir-locals.el` / `Local Variables:`
//! blocks may carry `eval` forms; `lispexp` evaluates nothing (ADR-0001), so
//! this crate resolves the *structural* entries and surfaces `eval` forms as
//! data without running them. The consumer likewise keeps the indent
//! *algorithm* (`calculate-lisp-indent`); this crate supplies the *data* it
//! runs on.

pub mod indent;
pub mod local_vars;
