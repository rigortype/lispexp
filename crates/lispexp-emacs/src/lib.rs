//! Emacs-specific integration data and interpreters for [`lispexp`] (ADR-0033).
//!
//! `lispexp` is a dialect-neutral S-expression *reader*. This companion crate
//! is the home for **Emacs-specific** knowledge that builds on it — data and
//! interpreters that are tied to one editor (Emacs), version-sensitive, and
//! kept out of the neutral core. It depends on `lispexp`, never the reverse.
//!
//! Scope, by axis (ADR-0033):
//! - **In scope:** Emacs-specific integration — the bundled standard indent
//!   table ([`indent`]); planned: a major-mode registry and a `.dir-locals.el`
//!   interpreter (an Emacs elisp data file `lispexp` already reads).
//! - **Out of scope, editor-*neutral*:** file-extension → dialect selection —
//!   that is deliberately the caller's, not Emacs-specific (lispexp ADR-0012).
//! - **Out of scope, *foreign format*:** EditorConfig and the like — not
//!   S-expressions; a consumer's rendering-policy concern.
//!
//! The consumer always keeps the indent *algorithm* (`calculate-lisp-indent`);
//! this crate supplies the *data* it runs on.

pub mod indent;
