//! A simple evaluator for Emacs `.dir-locals.el` — directory-local variables
//! (ADR-0033).
//!
//! `.dir-locals.el` is an elisp alist keyed by major mode:
//! `((MODE . ((VAR . VALUE) …)) …)`, where `nil` means *all modes*, plus an
//! optional per-subdirectory nesting `(("subdir" . ((MODE . VARS) …)) …)`. This
//! module reads it (via `lispexp`'s own Emacs Lisp reader) into raw
//! `name → value-text` bindings a consumer then interprets.
//!
//! **Read & interpret, never execute.** "Evaluator" here means it resolves the
//! *structure* (which mode / subdir a binding is under) — it does **not** run
//! elisp (`lispexp` ADR-0001). Each value is returned as its verbatim source
//! text, so an `(eval . (…))` entry is surfaced as a binding named `eval` whose
//! value is the unparsed form; it is never executed.
//!
//! ```
//! use lispexp_emacs::dir_locals::DirLocals;
//!
//! let dl = DirLocals::parse("((emacs-lisp-mode . ((indent-tabs-mode . t) (tab-width . 4))))");
//! let vars = dl.for_mode("emacs-lisp-mode");
//! assert_eq!(vars, vec![("indent-tabs-mode", "t"), ("tab-width", "4")]);
//! ```

use lispexp::{Datum, DatumKind, Options};

/// One resolved group of directory-local bindings: the variables under a
/// particular major mode (and optionally a particular subdirectory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirLocalEntry {
    /// The subdirectory this group is scoped to (`Some("subdir")`), or `None`
    /// for a top-level entry.
    pub subdir: Option<String>,
    /// The major mode this group applies to, or `None` for the `nil` = all-modes
    /// entry.
    pub mode: Option<String>,
    /// The `name → value-text` bindings, in source order. Values are verbatim
    /// (an `eval` entry's value is the unparsed form text, never executed).
    pub vars: Vec<(String, String)>,
}

/// A parsed `.dir-locals.el`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirLocals {
    entries: Vec<DirLocalEntry>,
}

impl DirLocals {
    /// Parse `.dir-locals.el` content. Malformed or non-alist input yields an
    /// empty result rather than an error (faithful to the reader-only stance).
    #[must_use]
    pub fn parse(content: &str) -> DirLocals {
        let parsed = lispexp::parse(content, &Options::emacs_lisp());
        let mut entries = Vec::new();
        if let Some(Datum {
            kind: DatumKind::List { items, .. },
            ..
        }) = parsed.data.first()
        {
            for entry in items {
                collect_entry(entry, None, content, &mut entries);
            }
        }
        DirLocals { entries }
    }

    /// Every resolved group, including subdirectory-scoped ones, in source order.
    #[must_use]
    pub fn entries(&self) -> &[DirLocalEntry] {
        &self.entries
    }

    /// The top-level (non-subdirectory) variables that apply to `mode`, in
    /// application order: the `nil` = all-modes bindings first, then `mode`'s own
    /// (so a mode-specific binding overrides an all-modes one — apply last-wins).
    #[must_use]
    pub fn for_mode(&self, mode: &str) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        for want_specific in [false, true] {
            for e in &self.entries {
                if e.subdir.is_some() {
                    continue;
                }
                let matches = match &e.mode {
                    None => !want_specific,                // nil pass
                    Some(m) => want_specific && m == mode, // specific pass
                };
                if matches {
                    out.extend(e.vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                }
            }
        }
        out
    }
}

/// Read one top-level alist element into `out`. A *string* key opens a
/// subdirectory scope whose value is itself a mode-alist; a symbol/`nil` key is
/// a mode group whose value is the `(VAR . VALUE)` alist.
fn collect_entry(entry: &Datum, subdir: Option<&str>, src: &str, out: &mut Vec<DirLocalEntry>) {
    let DatumKind::List { items, tail, .. } = &entry.kind else {
        return;
    };
    let Some(key) = items.first() else {
        return;
    };
    let rest = rest_items(items, tail);
    match &key.kind {
        // `("subdir" . ((MODE . VARS) …))` — recurse one level into the subdir.
        DatumKind::Str(s) if subdir.is_none() => {
            let dir = s.trim_matches('"');
            for mode_entry in rest {
                collect_entry(mode_entry, Some(dir), src, out);
            }
        }
        // `(MODE . ((VAR . VAL) …))` / `(MODE (VAR . VAL) …)`.
        DatumKind::Symbol(name) => {
            let mode = if *name == "nil" {
                None
            } else {
                Some((*name).to_string())
            };
            let vars = rest.iter().filter_map(|d| pair(d, src)).collect();
            out.push(DirLocalEntry {
                subdir: subdir.map(str::to_string),
                mode,
                vars,
            });
        }
        _ => {}
    }
}

/// The elements after the car of a `(CAR . REST)` datum: the tail list's items
/// when dotted (`(k . (a b))`), else the items after the first (`(k a b)`).
fn rest_items<'a, 't>(
    items: &'a [Datum<'t>],
    tail: &'a Option<Box<Datum<'t>>>,
) -> Vec<&'a Datum<'t>> {
    match tail {
        Some(t) => match &t.kind {
            DatumKind::List { items, .. } => items.iter().collect(),
            _ => Vec::new(),
        },
        None => items.iter().skip(1).collect(),
    }
}

/// A `(KEY . VALUE)` / `(KEY VALUE)` pair as verbatim `(name, value-text)`.
fn pair(datum: &Datum, src: &str) -> Option<(String, String)> {
    let DatumKind::List { items, tail, .. } = &datum.kind else {
        return None;
    };
    let key = items.first()?;
    let value = if let Some(t) = tail {
        t.as_ref()
    } else if items.len() == 2 {
        &items[1]
    } else {
        return None;
    };
    Some((text(key, src), text(value, src)))
}

fn text(datum: &Datum, src: &str) -> String {
    datum.span.text(src).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_mode_dotted_form() {
        let dl = DirLocals::parse("((emacs-lisp-mode . ((indent-tabs-mode . t) (tab-width . 3))))");
        assert_eq!(
            dl.for_mode("emacs-lisp-mode"),
            vec![("indent-tabs-mode", "t"), ("tab-width", "3")]
        );
    }

    #[test]
    fn non_dotted_mode_form() {
        // `(MODE (VAR . VAL) …)` — the form php-mode's own dir-locals uses.
        let dl = DirLocals::parse("((emacs-lisp-mode (tab-width . 5) (nameless-mode . t)))");
        assert_eq!(
            dl.for_mode("emacs-lisp-mode"),
            vec![("tab-width", "5"), ("nameless-mode", "t")]
        );
    }

    #[test]
    fn nil_applies_to_all_modes_first() {
        let dl = DirLocals::parse(
            "((nil . ((fill-column . 80))) (emacs-lisp-mode . ((fill-column . 70))))",
        );
        // nil first, then the mode-specific one (which the consumer applies last).
        assert_eq!(
            dl.for_mode("emacs-lisp-mode"),
            vec![("fill-column", "80"), ("fill-column", "70")]
        );
        // A mode with no specific entry still gets the nil bindings.
        assert_eq!(dl.for_mode("scheme-mode"), vec![("fill-column", "80")]);
    }

    #[test]
    fn eval_value_is_surfaced_verbatim_not_run() {
        let dl = DirLocals::parse("((nil . ((eval . (setq foo 1)))))");
        assert_eq!(dl.for_mode("any"), vec![("eval", "(setq foo 1)")]);
    }

    #[test]
    fn string_value_kept_verbatim() {
        let dl = DirLocals::parse("((nil . ((project-name . \"my app\"))))");
        assert_eq!(dl.for_mode("any"), vec![("project-name", "\"my app\"")]);
    }

    #[test]
    fn subdirectory_scope_is_captured_but_excluded_from_for_mode() {
        let dl =
            DirLocals::parse("((nil . ((a . 1))) (\"tests\" . ((emacs-lisp-mode . ((b . 2))))))");
        // for_mode only resolves top-level entries.
        assert_eq!(dl.for_mode("emacs-lisp-mode"), vec![("a", "1")]);
        // ...but the subdir group is retained for a consumer that wants it.
        let sub = dl
            .entries()
            .iter()
            .find(|e| e.subdir.as_deref() == Some("tests"))
            .expect("subdir entry");
        assert_eq!(sub.mode.as_deref(), Some("emacs-lisp-mode"));
        assert_eq!(sub.vars, vec![("b".to_string(), "2".to_string())]);
    }

    #[test]
    fn malformed_input_is_empty() {
        assert!(DirLocals::parse("").entries().is_empty());
        assert!(DirLocals::parse("not-a-list").entries().is_empty());
        assert!(DirLocals::parse("((emacs-lisp-mode))")
            .for_mode("emacs-lisp-mode")
            .is_empty());
    }
}
