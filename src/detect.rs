//! Opt-in, content-aware dialect detection (ADR-0034).
//!
//! Choosing which [`Dialect`]/[`Options`](crate::Options) to read a file with is
//! the caller's job — the reader stays passive (ADR-0012). This module is the
//! *opt-in layer* that ADR-0012 anticipated: call [`detect`] to *pick* a
//! dialect, then hand its `Options` to [`parse`](crate::parse). The reader's
//! one-parse-one-Options invariant is untouched.
//!
//! Detection resolves a **reader surface** ([`Dialect`]), never an
//! implementation/processor — a `.scm` file does not name whether it runs on
//! Gauche, Chez, or Guile (`CONTEXT.md`, ADR-0029).
//!
//! ```
//! use lispexp::detect::{detect, Confidence};
//! use lispexp::Dialect;
//!
//! let d = detect(Some("core.clj"), "(ns app.core)\n(defn f [x] x)");
//! assert_eq!(d.dialect, Some(Dialect::Clojure));
//! assert_eq!(d.confidence, Confidence::High); // unambiguous extension
//!
//! // Content decides when the extension can't: `#lang` ⇒ Racket.
//! let d = detect(None, "#lang racket/base\n(define x 1)");
//! assert_eq!(d.dialect, Some(Dialect::Racket));
//! ```

use crate::options::Dialect;

/// How sure a [`Detection`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Confidence {
    /// A `#lang`/shebang directive, or an extension owned by exactly one dialect.
    High,
    /// A shared extension resolved by a content signal.
    Medium,
    /// A content-only guess with no (or an unknown) extension.
    Low,
}

/// The result of [`detect`]: a best-effort dialect, a [`Confidence`], and a
/// short human-readable reason. `dialect` is `None` when no signal fired —
/// detection never fails closed into a wrong silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detection {
    /// The detected reader surface, or `None` if undetermined.
    pub dialect: Option<Dialect>,
    /// How sure the detection is.
    pub confidence: Confidence,
    /// Why this dialect (or nothing) was chosen.
    pub reason: &'static str,
}

impl Detection {
    const NONE: Detection = Detection {
        dialect: None,
        confidence: Confidence::Low,
        reason: "no extension or content signal",
    };

    const fn some(dialect: Dialect, confidence: Confidence, reason: &'static str) -> Detection {
        Detection {
            dialect: Some(dialect),
            confidence,
            reason,
        }
    }
}

impl Dialect {
    /// The file extensions (without the leading dot) conventionally associated
    /// with this dialect. Best-effort; some dialects (e.g. ISLisp) have no
    /// distinctive extension and return an empty slice.
    #[must_use]
    pub fn extensions(self) -> &'static [&'static str] {
        use Dialect::*;
        match self {
            Scheme => &["ss", "sld", "sls"],
            SchemeSuperset | Gauche | Mosh | Gambit => &["scm"],
            Guile => &["scm"],
            Racket => &["rkt", "rktl", "rktd"],
            Clojure => &["clj", "cljs", "cljc"],
            Phel => &["phel"],
            Edn => &["edn"],
            CommonLisp => &["lisp", "lsp", "cl"],
            EmacsLisp => &["el", "eld"],
            AutoLisp => &["lsp"],
            Janet => &["janet", "jdn"],
            Hy => &["hy"],
            Fennel => &["fnl"],
            Lfe => &["lfe"],
            Islisp => &[],
        }
    }

    /// The candidate dialects for a file extension (with or without a leading
    /// dot, case-insensitive), ordered by prior likelihood. A *shared* extension
    /// returns several candidates (`.scm` → superset then Guile; `.lsp` → Common
    /// Lisp then AutoLISP); an unknown extension returns an empty slice.
    #[must_use]
    pub fn from_extension(ext: &str) -> &'static [Dialect] {
        use Dialect::*;
        // Normalize: drop a leading dot and lowercase into a small buffer.
        let ext = ext.trim_start_matches('.');
        let lower = ext.to_ascii_lowercase();
        match lower.as_str() {
            "el" | "eld" => &[EmacsLisp],
            "scm" => &[SchemeSuperset, Guile],
            "ss" | "sld" | "sls" => &[Scheme],
            "rkt" | "rktl" | "rktd" => &[Racket],
            "clj" | "cljs" | "cljc" | "cljx" => &[Clojure],
            "edn" => &[Edn],
            "phel" => &[Phel],
            "fnl" => &[Fennel],
            "janet" | "jdn" => &[Janet],
            "hy" => &[Hy],
            "lfe" => &[Lfe],
            "cl" | "lisp" => &[CommonLisp],
            "lsp" => &[CommonLisp, AutoLisp],
            _ => &[],
        }
    }
}

/// Detect the dialect of a single file from its name and/or contents.
///
/// Precedence: a leading `#lang` directive, then a shebang interpreter, then the
/// file extension (disambiguated by content when the extension is shared), then
/// a content-only guess. See the module docs for the [`Confidence`] ladder.
#[must_use]
pub fn detect(filename: Option<&str>, source: &str) -> Detection {
    // 1. A `#lang` directive is definitive: Racket.
    if source.trim_start().starts_with("#lang") {
        return Detection::some(Dialect::Racket, Confidence::High, "#lang directive");
    }

    // 2. A shebang naming a known interpreter.
    if let Some(d) = shebang_dialect(source) {
        return Detection::some(d, Confidence::High, "shebang interpreter");
    }

    // 3. The file extension.
    if let Some(name) = filename {
        if let Some(ext) = name.rsplit('.').next().filter(|e| *e != name) {
            let candidates = Dialect::from_extension(ext);
            match candidates {
                [only] => return Detection::some(*only, Confidence::High, "unambiguous extension"),
                [_, ..] => {
                    if let Some(d) = disambiguate(candidates, source) {
                        return Detection::some(d, Confidence::Medium, "extension + content");
                    }
                    return Detection::some(
                        candidates[0],
                        Confidence::Medium,
                        "extension (default)",
                    );
                }
                [] => {}
            }
        }
    }

    // 4. Content-only markers (unknown/absent extension).
    if let Some(d) = content_dialect(source) {
        return Detection::some(d, Confidence::Low, "content marker");
    }

    Detection::NONE
}

/// Aggregate per-file [`detect`] results into one project dialect.
///
/// Each file votes for its detected dialect, weighted by confidence (`High` 3,
/// `Medium` 2, `Low` 1); undetected files abstain. The highest total wins, ties
/// broken deterministically by [`Dialect::ALL`] order — so the result does not
/// depend on the iteration order of `files`. Returns `None` if nothing was
/// detected.
#[must_use]
pub fn detect_project<I, N, S>(files: I) -> Option<Dialect>
where
    I: IntoIterator<Item = (N, S)>,
    N: AsRef<str>,
    S: AsRef<str>,
{
    let mut scores: Vec<(Dialect, u32)> = Vec::new();
    for (name, source) in files {
        let d = detect(Some(name.as_ref()), source.as_ref());
        let (Some(dialect), weight) = (d.dialect, confidence_weight(d.confidence)) else {
            continue;
        };
        match scores.iter_mut().find(|(k, _)| *k == dialect) {
            Some((_, s)) => *s += weight,
            None => scores.push((dialect, weight)),
        }
    }
    // Argmax by score, tie-broken by position in `Dialect::ALL` (stable, so the
    // outcome is independent of `files` order).
    scores
        .into_iter()
        .max_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| all_index(b.0).cmp(&all_index(a.0)))
        })
        .map(|(d, _)| d)
}

fn confidence_weight(c: Confidence) -> u32 {
    match c {
        Confidence::High => 3,
        Confidence::Medium => 2,
        Confidence::Low => 1,
    }
}

fn all_index(d: Dialect) -> usize {
    Dialect::ALL
        .iter()
        .position(|&x| x == d)
        .unwrap_or(usize::MAX)
}

/// Map a shebang interpreter basename to a dialect, if the first line is a `#!`.
fn shebang_dialect(source: &str) -> Option<Dialect> {
    let first = source.lines().next()?;
    let rest = first.strip_prefix("#!")?;
    // The interpreter is the last path-like token whose basename we recognize
    // (handles both `#!/usr/bin/guile` and `#!/usr/bin/env guile`).
    for tok in rest.split(|c: char| c.is_whitespace()).rev() {
        let base = tok.rsplit(['/', '\\']).next().unwrap_or(tok);
        let d = match base {
            "guile" => Dialect::Guile,
            "gosh" => Dialect::SchemeSuperset,
            "scheme" | "chez" | "chezscheme" | "petite" | "csi" | "chibi-scheme" => Dialect::Scheme,
            "racket" => Dialect::Racket,
            "sbcl" | "ccl" | "clisp" | "ecl" | "abcl" => Dialect::CommonLisp,
            "clojure" | "clj" | "bb" => Dialect::Clojure,
            "janet" => Dialect::Janet,
            "hy" => Dialect::Hy,
            "fennel" => Dialect::Fennel,
            "emacs" => Dialect::EmacsLisp,
            "lfe" | "lfescript" => Dialect::Lfe,
            _ => continue,
        };
        return Some(d);
    }
    None
}

/// Pick among the candidates of a *shared* extension using content signals.
fn disambiguate(candidates: &[Dialect], source: &str) -> Option<Dialect> {
    // `.scm` → Guile vs the tolerant superset.
    if candidates.contains(&Dialect::Guile) {
        if source.contains("(define-module") || source.contains("(use-modules") {
            return Some(Dialect::Guile);
        }
        return Some(Dialect::SchemeSuperset);
    }
    // `.lsp` → AutoLISP vs Common Lisp.
    if candidates.contains(&Dialect::AutoLisp) {
        if source.contains("(defun c:")
            || source.contains("(vl-")
            || source.contains("(vlax-")
            || source.contains("(command ")
        {
            return Some(Dialect::AutoLisp);
        }
        return Some(Dialect::CommonLisp);
    }
    None
}

/// A content-only guess for the few dialects with an unambiguous structural
/// marker. Deliberately conservative: `(defn …)` alone is *not* used, since it
/// is shared by Clojure, Janet, Hy, and Fennel.
fn content_dialect(source: &str) -> Option<Dialect> {
    if source.contains("(define-library")
        || source.contains("(import (scheme")
        || source.contains("(import (rnrs")
    {
        return Some(Dialect::Scheme);
    }
    if source.contains("(defpackage") || source.contains("(in-package") {
        return Some(Dialect::CommonLisp);
    }
    if source.contains("lexical-binding:")
        || source.contains(";;;###autoload")
        || source.contains("(provide '")
    {
        return Some(Dialect::EmacsLisp);
    }
    if source.contains("(ns ") {
        return Some(Dialect::Clojure);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unambiguous_extension_is_high() {
        assert_eq!(
            detect(Some("init.el"), "(defun f ())"),
            Detection::some(
                Dialect::EmacsLisp,
                Confidence::High,
                "unambiguous extension"
            )
        );
        assert_eq!(
            detect(Some("core.clj"), "(ns app)").dialect,
            Some(Dialect::Clojure)
        );
        assert_eq!(
            detect(Some("data.edn"), "{:a 1}").dialect,
            Some(Dialect::Edn)
        );
    }

    #[test]
    fn lang_directive_beats_everything() {
        let d = detect(Some("weird.txt"), "#lang typed/racket\n(define x 1)");
        assert_eq!(d.dialect, Some(Dialect::Racket));
        assert_eq!(d.confidence, Confidence::High);
        // Leading whitespace before #lang still counts.
        assert_eq!(
            detect(None, "  #lang racket").dialect,
            Some(Dialect::Racket)
        );
    }

    #[test]
    fn shebang_env_and_direct() {
        assert_eq!(
            detect(None, "#!/usr/bin/env fennel\n(local x 1)").dialect,
            Some(Dialect::Fennel)
        );
        assert_eq!(
            detect(None, "#!/usr/bin/guile -s\n!#\n(display 1)").dialect,
            Some(Dialect::Guile)
        );
        assert_eq!(
            detect(Some("script"), "#!/usr/bin/sbcl --script\n(print 1)").dialect,
            Some(Dialect::CommonLisp)
        );
    }

    #[test]
    fn shared_scm_extension_disambiguates_guile() {
        // Plain .scm → the tolerant superset.
        let d = detect(Some("lib.scm"), "(define (f x) x)");
        assert_eq!(d.dialect, Some(Dialect::SchemeSuperset));
        assert_eq!(d.confidence, Confidence::Medium);
        // Guile module form → Guile.
        let d = detect(Some("lib.scm"), "(define-module (a b))\n(define x 1)");
        assert_eq!(d.dialect, Some(Dialect::Guile));
    }

    #[test]
    fn shared_lsp_extension_disambiguates_autolisp() {
        assert_eq!(
            detect(Some("cmd.lsp"), "(defun c:hello () (princ))").dialect,
            Some(Dialect::AutoLisp)
        );
        assert_eq!(
            detect(Some("pkg.lsp"), "(defpackage :app)").dialect,
            Some(Dialect::CommonLisp)
        );
        // A .lsp with no distinctive content defaults to the first candidate.
        assert_eq!(
            detect(Some("x.lsp"), "(+ 1 2)").dialect,
            Some(Dialect::CommonLisp)
        );
    }

    #[test]
    fn content_only_markers() {
        assert_eq!(
            detect(None, "(define-library (foo) (export bar))").dialect,
            Some(Dialect::Scheme)
        );
        assert_eq!(
            detect(None, "(in-package :cl-user)").dialect,
            Some(Dialect::CommonLisp)
        );
        assert_eq!(
            detect(None, ";;; -*- lexical-binding: t -*-\n(defun f ())").dialect,
            Some(Dialect::EmacsLisp)
        );
        let d = detect(None, "(define-library (x))");
        assert_eq!(d.confidence, Confidence::Low);
    }

    #[test]
    fn nothing_detected_is_none_not_a_wrong_guess() {
        let d = detect(Some("mystery.txt"), "(+ 1 2)");
        assert_eq!(d.dialect, None);
        assert_eq!(detect(None, "(+ 1 2)").dialect, None);
    }

    #[test]
    fn extension_registry_round_trips_and_shares() {
        assert_eq!(Dialect::from_extension("el"), &[Dialect::EmacsLisp]);
        assert_eq!(Dialect::from_extension(".EL"), &[Dialect::EmacsLisp]); // dot + case
        assert_eq!(
            Dialect::from_extension("scm"),
            &[Dialect::SchemeSuperset, Dialect::Guile]
        );
        assert_eq!(
            Dialect::from_extension("lsp"),
            &[Dialect::CommonLisp, Dialect::AutoLisp]
        );
        assert!(Dialect::from_extension("txt").is_empty());
        assert!(Dialect::EmacsLisp.extensions().contains(&"el"));
    }

    #[test]
    fn project_aggregates_order_independently() {
        let files = [
            ("a.clj", "(ns a)"),
            ("b.clj", "(ns b)"),
            ("weird.el", "(defun f ())"),
        ];
        assert_eq!(detect_project(files), Some(Dialect::Clojure));
        // Reversed order → same winner (order-independent aggregation).
        let mut rev = files;
        rev.reverse();
        assert_eq!(detect_project(rev), Some(Dialect::Clojure));
        assert_eq!(detect_project(Vec::<(&str, &str)>::new()), None);
    }
}
