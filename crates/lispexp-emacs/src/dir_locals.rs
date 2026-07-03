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
    ///
    /// Equivalent to [`for_path`](DirLocals::for_path) with an empty path — it
    /// ignores subdirectory-scoped groups. Use `for_path` to resolve a file
    /// under a subdirectory.
    #[must_use]
    pub fn for_mode(&self, mode: &str) -> Vec<(&str, &str)> {
        self.for_path(mode, "")
    }

    /// The variables that apply to a file at `relpath` (relative to the
    /// `.dir-locals.el`'s directory, forward-slash separated) in major mode
    /// `mode`.
    ///
    /// Scopes are applied outer-to-inner so the nearest wins under last-wins:
    /// first the top-level groups, then each `("subdir" . …)` whose directory is
    /// an ancestor of `relpath`, ordered shallowest-first. Within each scope the
    /// `nil` = all-modes bindings come before `mode`'s own. A subdir matches on a
    /// directory boundary — `"foo"` covers `foo/bar.el` but not `foobar.el`.
    #[must_use]
    pub fn for_path(&self, mode: &str, relpath: &str) -> Vec<(&str, &str)> {
        let rel = relpath.trim_start_matches("./");
        // Distinct matching subdirs, shallowest-first (stable within equal depth).
        let mut subdirs: Vec<&str> = Vec::new();
        for e in &self.entries {
            if let Some(s) = e.subdir.as_deref() {
                if under_dir(rel, s) && !subdirs.contains(&s) {
                    subdirs.push(s);
                }
            }
        }
        subdirs.sort_by_key(|s| depth(s));

        let mut out = Vec::new();
        self.emit_scope(None, mode, &mut out);
        for s in subdirs {
            self.emit_scope(Some(s), mode, &mut out);
        }
        out
    }

    /// Append the bindings of one scope (`None` = top level, `Some(dir)` = a
    /// subdir), `nil`-mode first then `mode`-specific.
    fn emit_scope<'a>(
        &'a self,
        scope: Option<&str>,
        mode: &str,
        out: &mut Vec<(&'a str, &'a str)>,
    ) {
        for want_specific in [false, true] {
            for e in &self.entries {
                if e.subdir.as_deref() != scope {
                    continue;
                }
                let matches = match &e.mode {
                    None => !want_specific,
                    Some(m) => want_specific && m == mode,
                };
                if matches {
                    out.extend(e.vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                }
            }
        }
    }
}

/// Whether a file at `rel` sits under the directory `dir` (matching on a `/`
/// boundary, trailing slash on `dir` ignored). An empty `dir` matches nothing.
fn under_dir(rel: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches('/');
    if dir.is_empty() {
        return false;
    }
    rel.strip_prefix(dir)
        .is_some_and(|after| after.starts_with('/'))
}

/// The directory depth (segment count) of a subdir key, for shallow-first order.
fn depth(dir: &str) -> usize {
    dir.trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .count()
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
    fn for_path_applies_matching_subdir_after_top_level() {
        let dl = DirLocals::parse("((nil . ((a . 1))) (\"tests\" . ((nil . ((a . 2))))))");
        // A file under tests/ gets top-level then the subdir (last-wins → 2).
        assert_eq!(
            dl.for_path("emacs-lisp-mode", "tests/foo.el"),
            vec![("a", "1"), ("a", "2")]
        );
        // A file outside tests/ gets only the top-level binding.
        assert_eq!(dl.for_path("emacs-lisp-mode", "foo.el"), vec![("a", "1")]);
        // for_mode ignores subdirs entirely.
        assert_eq!(dl.for_mode("emacs-lisp-mode"), vec![("a", "1")]);
    }

    #[test]
    fn deeper_subdir_overrides_shallower() {
        let dl = DirLocals::parse(
            "((\"src/inner\" . ((nil . ((a . 3))))) (\"src\" . ((nil . ((a . 2))))))",
        );
        // Regardless of source order, shallower (src) applies before deeper (src/inner).
        assert_eq!(
            dl.for_path("m", "src/inner/f.el"),
            vec![("a", "2"), ("a", "3")]
        );
        assert_eq!(dl.for_path("m", "src/f.el"), vec![("a", "2")]);
    }

    #[test]
    fn subdir_matches_on_a_directory_boundary() {
        let dl = DirLocals::parse("((\"foo\" . ((nil . ((a . 1))))))");
        assert_eq!(dl.for_path("m", "foo/bar.el"), vec![("a", "1")]);
        // "foobar.el" is not under directory "foo".
        assert!(dl.for_path("m", "foobar.el").is_empty());
        // A trailing slash on the key is tolerated.
        let dl2 = DirLocals::parse("((\"foo/\" . ((nil . ((a . 1))))))");
        assert_eq!(dl2.for_path("m", "foo/bar.el"), vec![("a", "1")]);
    }

    #[test]
    fn mode_specific_subdir_entry_respects_mode() {
        let dl = DirLocals::parse("((\"tests\" . ((emacs-lisp-mode . ((a . 9))))))");
        assert_eq!(
            dl.for_path("emacs-lisp-mode", "tests/f.el"),
            vec![("a", "9")]
        );
        assert!(dl.for_path("scheme-mode", "tests/f.el").is_empty());
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
