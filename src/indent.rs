//! Indent specs: a first-class `symbol → IndentSpec` table (ADR-0022).
//!
//! Per-symbol indentation metadata — Emacs's `(declare (indent …))` and
//! `lisp-indent-function` — held **independently** of the definition-form
//! registry ([`crate::annotate`]). Indent specs are not a property of
//! definitions: control and binding macros like `when`, `dolist`, and
//! `with-slots` carry indent specs too, and none of them is a def-form.
//!
//! Harvesting is Emacs-Lisp-specific for now (ADR-0022): the source of indent
//! specs is elisp's `declare`/`lisp-indent-function`. The [`IndentSpec`] *type*
//! is general, but other dialects' specs (Clojure's `:style/indent`, …) are the
//! consumer's to supply.
//!
//! Consistent with reader-only scope (ADR-0001): lispexp reads declared metadata
//! and hands it over verbatim-typed — it never runs an indent function.

use std::collections::HashMap;

use crate::datum::{Datum, DatumKind, Prefix};
use crate::options::Options;
use crate::reader::parse;

/// A per-symbol indentation spec (ADR-0022).
///
/// The known elisp grammar is captured by type; anything unexpected falls back
/// to [`IndentSpec::Raw`], staying faithful. `Function` holds a function *name
/// only* — lispexp neither resolves nor runs it (reader-only).
#[derive(Debug, Clone, PartialEq)]
pub enum IndentSpec<'a> {
    /// An integer indent (`(declare (indent 2))`).
    Number(u32),
    /// The special `defun` indentation (`(declare (indent defun))`).
    Defun,
    /// A custom indent-function *name* — not resolved or run.
    Function(&'a str),
    /// Any other spec, kept verbatim.
    Raw(Datum<'a>),
}

/// A first-class `symbol → IndentSpec` table, independent of the definition
/// registry (ADR-0022).
#[derive(Debug, Clone, Default)]
pub struct IndentTable<'a> {
    specs: HashMap<&'a str, IndentSpec<'a>>,
}

impl<'a> IndentTable<'a> {
    /// An empty table.
    pub fn new() -> Self {
        IndentTable::default()
    }

    /// Insert a spec, overwriting any existing entry for `symbol`.
    pub fn insert(&mut self, symbol: &'a str, spec: IndentSpec<'a>) {
        self.specs.insert(symbol, spec);
    }

    /// The indent spec for `symbol`, if any.
    pub fn get(&self, symbol: &str) -> Option<&IndentSpec<'a>> {
        self.specs.get(symbol)
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Iterate `(symbol, spec)` entries.
    pub fn iter(&self) -> impl Iterator<Item = (&&'a str, &IndentSpec<'a>)> {
        self.specs.iter()
    }
}

/// Harvest indent specs from Emacs Lisp `source` into a fresh [`IndentTable`]
/// (ADR-0022).
///
/// Collects both signals across the whole source, for definitions and
/// non-definitions alike:
/// - `(declare (indent SPEC))` inside a `def…`/`cl-def…` form → the form's name;
/// - `(put 'SYM 'lisp-indent-function SPEC)` and the `function-put` equivalent.
pub fn harvest_indent_specs(source: &str) -> IndentTable<'_> {
    let parsed = parse(source, &Options::emacs_lisp());
    let mut table = IndentTable::new();
    for form in &parsed.data {
        harvest_form(form, &mut table);
    }
    table
}

fn harvest_form<'a>(form: &Datum<'a>, table: &mut IndentTable<'a>) {
    let DatumKind::List { items, .. } = &form.kind else {
        return;
    };
    if let Some(head) = list_head(items) {
        harvest_put(head, items, table);
        harvest_declare(head, items, table);
    }
    // Recurse: indent-affecting forms may be nested (progn, eval-when-compile…).
    for item in items {
        harvest_form(item, table);
    }
}

/// A `(put 'SYM 'lisp-indent-function SPEC)` / `(function-put …)` form.
fn harvest_put<'a>(head: &str, items: &[Datum<'a>], table: &mut IndentTable<'a>) {
    if head != "put" && head != "function-put" {
        return;
    }
    let Some(symbol) = items.get(1).and_then(quoted_symbol) else {
        return;
    };
    let Some(prop) = items.get(2).and_then(quoted_symbol) else {
        return;
    };
    if prop != "lisp-indent-function" && prop != "lisp-indent-hook" {
        return;
    }
    if let Some(spec) = items.get(3) {
        table.insert(symbol, indent_spec(spec));
    }
}

/// A `(def… NAME … (declare … (indent SPEC) …) …)` form.
fn harvest_declare<'a>(head: &str, items: &[Datum<'a>], table: &mut IndentTable<'a>) {
    if !(head.starts_with("def") || head.starts_with("cl-def")) {
        return;
    }
    let Some(name) = items.get(1).and_then(as_symbol) else {
        return;
    };
    // Find a `(declare …)` among the form's items, then an `(indent SPEC)`.
    for item in &items[2.min(items.len())..] {
        let Some(decl) = list_head_items(item) else {
            continue;
        };
        if list_head(decl) != Some("declare") {
            continue;
        }
        for clause in &decl[1..] {
            if let Some(cl) = list_head_items(clause) {
                if list_head(cl) == Some("indent") {
                    if let Some(spec) = cl.get(1) {
                        table.insert(name, indent_spec(spec));
                    }
                }
            }
        }
    }
}

/// Classify an indent spec datum. Unwraps a leading quote (for `put` forms).
fn indent_spec<'a>(spec: &Datum<'a>) -> IndentSpec<'a> {
    match &spec.kind {
        DatumKind::Number(n) => n
            .parse::<u32>()
            .map(IndentSpec::Number)
            .unwrap_or_else(|_| IndentSpec::Raw(spec.clone())),
        DatumKind::Symbol("defun") => IndentSpec::Defun,
        DatumKind::Symbol(name) => IndentSpec::Function(name),
        DatumKind::Prefixed {
            prefix: Prefix::Quote,
            inner,
            ..
        } => indent_spec(inner),
        _ => IndentSpec::Raw(spec.clone()),
    }
}

/// The inner symbol of a `'sym` quoted-symbol datum.
fn quoted_symbol<'a>(datum: &Datum<'a>) -> Option<&'a str> {
    if let DatumKind::Prefixed {
        prefix: Prefix::Quote,
        inner,
        ..
    } = &datum.kind
    {
        return as_symbol(inner);
    }
    None
}

fn as_symbol<'a>(datum: &Datum<'a>) -> Option<&'a str> {
    match datum.kind {
        DatumKind::Symbol(s) => Some(s),
        _ => None,
    }
}

/// The items of `datum` if it is a list.
fn list_head_items<'a, 'd>(datum: &'d Datum<'a>) -> Option<&'d [Datum<'a>]> {
    match &datum.kind {
        DatumKind::List { items, .. } => Some(items),
        _ => None,
    }
}

/// The head symbol of a list's items.
fn list_head<'a>(items: &[Datum<'a>]) -> Option<&'a str> {
    items.first().and_then(as_symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_indent_number() {
        let table =
            harvest_indent_specs("(defmacro my-when (c &rest body) (declare (indent 1)) body)");
        assert_eq!(table.get("my-when"), Some(&IndentSpec::Number(1)));
    }

    #[test]
    fn declare_indent_defun() {
        let table = harvest_indent_specs("(defmacro deffoo (name) (declare (indent defun)) name)");
        assert_eq!(table.get("deffoo"), Some(&IndentSpec::Defun));
    }

    #[test]
    fn declare_indent_function_name() {
        let table = harvest_indent_specs("(defmacro m (a) (declare (indent my-indent-fn)) a)");
        assert_eq!(table.get("m"), Some(&IndentSpec::Function("my-indent-fn")));
    }

    #[test]
    fn put_lisp_indent_function() {
        let table = harvest_indent_specs("(put 'with-foo 'lisp-indent-function 2)");
        assert_eq!(table.get("with-foo"), Some(&IndentSpec::Number(2)));
    }

    #[test]
    fn put_quoted_defun_spec() {
        let table = harvest_indent_specs("(put 'my-form 'lisp-indent-function 'defun)");
        assert_eq!(table.get("my-form"), Some(&IndentSpec::Defun));
    }

    #[test]
    fn function_put_signal() {
        let table = harvest_indent_specs("(function-put 'my-macro 'lisp-indent-function 3)");
        assert_eq!(table.get("my-macro"), Some(&IndentSpec::Number(3)));
    }

    #[test]
    fn non_definition_macro_gets_indent() {
        // A control macro that is not a def-form still carries an indent spec
        // (via put) — the table serves it, which a FormSpec-only design could not.
        let table = harvest_indent_specs("(put 'dolist 'lisp-indent-function 1)");
        assert_eq!(table.get("dolist"), Some(&IndentSpec::Number(1)));
    }

    #[test]
    fn raw_fallback_for_list_spec() {
        let table = harvest_indent_specs("(put 'weird 'lisp-indent-function '(1 2))");
        assert!(matches!(table.get("weird"), Some(IndentSpec::Raw(_))));
    }

    #[test]
    fn empty_when_no_specs() {
        let table = harvest_indent_specs("(defun plain (x) x)");
        assert!(table.is_empty());
    }
}
