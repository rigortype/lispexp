//! Emacs **file-local variables**: the leading `-*- … -*-` header cookie and the
//! trailing `Local Variables:` … `End:` block (ADR-0033).
//!
//! Emacs lets a file carry per-file settings two ways — a prop line at the top
//! (`;;; foo.el -*- mode: lisp; lexical-binding: t -*-`) and a `Local
//! Variables:` block at the bottom. This module reads both into raw
//! `name → value-text` bindings that a consumer (a formatter, a linter) then
//! interprets.
//!
//! **Read & interpret, never execute.** A value is returned as its verbatim
//! source text — Emacs would `read` it as an elisp expression, but this crate
//! evaluates nothing (lispexp ADR-0001). In particular an `eval: (…)` entry is
//! surfaced as a binding named `eval` whose value is the unparsed form text; it
//! is **never run**. A consumer interprets the handful of variables it cares
//! about (`indent-tabs-mode`, `tab-width`, `lexical-binding`, …).
//!
//! ```
//! use lispexp_emacs::local_vars::file_locals;
//!
//! let src = ";;; x.el -*- lexical-binding: t; tab-width: 4 -*-\n(defun f ())\n";
//! let fl = file_locals(src);
//! assert_eq!(fl.get("lexical-binding"), Some("t"));
//! assert_eq!(fl.get("tab-width"), Some("4"));
//! ```

/// The file-local variables read from a source buffer — raw `name → value-text`
/// bindings in application order (the header cookie first, then the `Local
/// Variables:` block), so a later binding for the same name overrides an
/// earlier one, as in Emacs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileLocals {
    vars: Vec<(String, String)>,
}

impl FileLocals {
    /// Every binding, in application order. Duplicate names are kept (the last
    /// one is the effective value — see [`get`](FileLocals::get)); an `eval`
    /// entry appears here verbatim and is never executed.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The effective (last-wins) raw value text for `name`, if bound.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Whether any file-local variable was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// The number of bindings (including duplicates).
    #[must_use]
    pub fn len(&self) -> usize {
        self.vars.len()
    }
}

/// Read all file-local variables from `source`: the `-*- … -*-` header cookie
/// followed by the `Local Variables:` … `End:` block.
#[must_use]
pub fn file_locals(source: &str) -> FileLocals {
    let mut vars = Vec::new();
    read_header(source, &mut vars);
    read_footer(source, &mut vars);
    FileLocals { vars }
}

/// The `-*- … -*-` prop line: the first line, or the second if the first is a
/// shebang. Two forms — a bare major-mode name (`-*- lisp -*-`, normalized to a
/// `mode` binding) or `var: val; …` pairs.
fn read_header(source: &str, out: &mut Vec<(String, String)>) {
    let mut lines = source.lines();
    let first = lines.next().unwrap_or_default();
    let header = if first.starts_with("#!") {
        lines.next().unwrap_or_default()
    } else {
        first
    };
    let Some(inner) = between(header, "-*-", "-*-") else {
        return;
    };
    if inner.contains(':') {
        for part in inner.split(';') {
            if let Some((k, v)) = part.split_once(':') {
                push(out, k, v);
            }
        }
    } else {
        // `-*- ModeName -*-` is shorthand for `-*- mode: ModeName -*-`.
        let mode = inner.trim();
        if !mode.is_empty() {
            push(out, "mode", mode);
        }
    }
}

/// The trailing `Local Variables:` … `End:` block. Its lines share the comment
/// prefix that precedes the `Local Variables:` marker on its own line.
fn read_footer(source: &str, out: &mut Vec<(String, String)>) {
    let Some(marker) = source.rfind("Local Variables:") else {
        return;
    };
    let line_start = source[..marker].rfind('\n').map_or(0, |i| i + 1);
    let prefix = &source[line_start..marker];
    for line in source[marker..].lines().skip(1) {
        let body = line
            .strip_prefix(prefix)
            .unwrap_or_else(|| line.trim_start())
            .trim();
        if body.starts_with("End:") {
            break;
        }
        if let Some((k, v)) = body.split_once(':') {
            push(out, k, v);
        }
    }
}

fn push(out: &mut Vec<(String, String)>, key: &str, val: &str) {
    let key = key.trim();
    if !key.is_empty() {
        out.push((key.to_string(), val.trim().to_string()));
    }
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let end = s[start..].find(close)? + start;
    Some(&s[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_variable_form() {
        let fl = file_locals(";;; x -*- indent-tabs-mode: t; tab-width: 4 -*-\n(foo)\n");
        assert_eq!(fl.get("indent-tabs-mode"), Some("t"));
        assert_eq!(fl.get("tab-width"), Some("4"));
    }

    #[test]
    fn header_bare_mode_form_normalizes_to_mode() {
        let fl = file_locals("-*- lisp -*-\n(foo)");
        assert_eq!(fl.get("mode"), Some("lisp"));
    }

    #[test]
    fn shebang_line_is_skipped_for_header() {
        let fl = file_locals("#!/usr/bin/emacs --script\n;; -*- tab-width: 2 -*-\n");
        assert_eq!(fl.get("tab-width"), Some("2"));
    }

    #[test]
    fn footer_block_with_comment_prefix() {
        let fl = file_locals("(foo)\n;; Local Variables:\n;; indent-tabs-mode: t\n;; End:\n");
        assert_eq!(fl.get("indent-tabs-mode"), Some("t"));
    }

    #[test]
    fn footer_overrides_header_last_wins() {
        let src = ";;; -*- tab-width: 2 -*-\n(x)\n;; Local Variables:\n;; tab-width: 8\n;; End:\n";
        let fl = file_locals(src);
        assert_eq!(fl.get("tab-width"), Some("8"));
        // Both bindings are retained in order.
        assert_eq!(fl.len(), 2);
    }

    #[test]
    fn eval_entry_is_surfaced_as_data_not_executed() {
        let src = "(x)\n;; Local Variables:\n;; eval: (setq foo 1)\n;; End:\n";
        let fl = file_locals(src);
        assert_eq!(fl.get("eval"), Some("(setq foo 1)"));
    }

    #[test]
    fn no_locals_is_empty() {
        let fl = file_locals("(defun f ())\n");
        assert!(fl.is_empty());
        assert_eq!(fl.get("tab-width"), None);
    }

    #[test]
    fn string_value_kept_verbatim() {
        // The consumer, not this reader, strips quotes / reads the elisp value.
        let fl = file_locals(";;; -*- foo: \"bar baz\" -*-\n");
        assert_eq!(fl.get("foo"), Some("\"bar baz\""));
    }
}
