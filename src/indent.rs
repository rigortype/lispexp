//! Indent specs: a first-class `symbol → IndentSpec` table (ADR-0022).
//!
//! Per-symbol indentation metadata — Emacs's `(declare (indent …))` and
//! `lisp-indent-function` — held **independently** of the definition-form
//! registry ([`crate::annotate`]). Indent specs are not a property of
//! definitions: control and binding macros like `when`, `dolist`, and
//! `with-slots` carry indent specs too, and none of them is a def-form.
//!
//! The table is **owned** (no borrow of the harvested source): its whole
//! purpose is to be merged across many files and outlive them all — see
//! [`harvest_indent_specs_into`] and [`IndentTable::extend`]. An indent table
//! is dozens of tiny entries per file; owning costs nothing, unlike the Datum
//! tree (ADR-0008).
//!
//! Harvesting is Emacs-Lisp-specific for now (ADR-0022): the source of indent
//! specs is elisp's `declare`/`lisp-indent-function`. The [`IndentSpec`] *type*
//! is general, but other dialects' specs (Clojure's `:style/indent`, …) are the
//! consumer's to supply.
//!
//! Consistent with reader-only scope (ADR-0001): lispexp reads declared metadata
//! and hands it over verbatim-typed — it never runs an indent function.
//!
//! # Example
//!
//! ```
//! use lispexp::indent::{harvest_indent_specs, IndentSpec};
//!
//! let table = harvest_indent_specs("(defmacro with-x (x) (declare (indent 1)) x)");
//! assert_eq!(table.get("with-x"), Some(&IndentSpec::Number(1)));
//! ```

use std::collections::HashMap;

use crate::datum::{Datum, DatumKind, Delim, Prefix};
use crate::options::Options;
use crate::reader::parse;

/// A per-symbol indentation spec (ADR-0022).
///
/// The known elisp grammar is captured by type; anything unexpected falls back
/// to [`IndentSpec::Raw`], staying faithful. `Function` holds a function *name
/// only* — lispexp neither resolves nor runs it (reader-only).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IndentSpec {
    /// An integer indent (`(declare (indent 2))`).
    Number(u32),
    /// The special `defun` indentation (`(declare (indent defun))`).
    Defun,
    /// A custom indent-function *name* — not resolved or run.
    Function(String),
    /// Any other spec, kept as its verbatim source text.
    Raw(String),
}

/// A first-class `symbol → IndentSpec` table, independent of the definition
/// registry (ADR-0022). Owned: merge tables from many files and keep one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndentTable {
    specs: HashMap<String, IndentSpec>,
}

impl IndentTable {
    /// An empty table.
    pub fn new() -> Self {
        IndentTable::default()
    }

    /// Insert a spec, overwriting any existing entry for `symbol`.
    pub fn insert(&mut self, symbol: impl Into<String>, spec: IndentSpec) {
        self.specs.insert(symbol.into(), spec);
    }

    /// The indent spec for `symbol`, if any.
    pub fn get(&self, symbol: &str) -> Option<&IndentSpec> {
        self.specs.get(symbol)
    }

    /// Remove and return the spec for `symbol`, if any.
    pub fn remove(&mut self, symbol: &str) -> Option<IndentSpec> {
        self.specs.remove(symbol)
    }

    /// Merge `other` into `self`; on a symbol collision, `other`'s spec wins.
    pub fn merge(&mut self, other: IndentTable) {
        self.specs.extend(other.specs);
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Iterate `(symbol, spec)` entries (no defined order).
    pub fn iter(&self) -> impl Iterator<Item = (&str, &IndentSpec)> {
        self.specs.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl Extend<(String, IndentSpec)> for IndentTable {
    fn extend<T: IntoIterator<Item = (String, IndentSpec)>>(&mut self, iter: T) {
        self.specs.extend(iter);
    }
}

/// Harvest indent specs from Emacs Lisp `source` into a fresh [`IndentTable`]
/// (ADR-0022). For a many-file harvest, prefer [`harvest_indent_specs_into`].
#[must_use]
pub fn harvest_indent_specs(source: &str) -> IndentTable {
    let mut table = IndentTable::new();
    harvest_indent_specs_into(source, &mut table);
    table
}

/// Harvest indent specs from Emacs Lisp `source` into an existing table,
/// so specs from many files accumulate into one merged table. Returns the
/// number of entries added or replaced.
///
/// Collects both signals across the whole source, for definitions and
/// non-definitions alike:
/// - `(declare (indent SPEC))` inside a `def…`/`cl-def…` form → the form's name;
/// - `(put 'SYM 'lisp-indent-function SPEC)` and the `function-put` equivalent.
pub fn harvest_indent_specs_into(source: &str, table: &mut IndentTable) -> usize {
    let parsed = parse(source, &Options::emacs_lisp());
    let before = table.len();
    let mut replaced = 0;
    for form in &parsed.data {
        harvest_form(source, form, table, &mut replaced);
    }
    table.len() - before + replaced
}

fn harvest_form(source: &str, form: &Datum<'_>, table: &mut IndentTable, replaced: &mut usize) {
    let DatumKind::List { delim, items, .. } = &form.kind else {
        return;
    };
    // Square/curly lists are data in elisp (vectors); a `(put …)` inside one is
    // never executed — don't harvest from it.
    if *delim != Delim::Round {
        return;
    }
    if let Some(head) = form.head_symbol() {
        harvest_put(source, head, items, table, replaced);
        harvest_declare(source, head, items, table, replaced);
    }
    // Recurse: indent-affecting forms may be nested (progn, eval-when-compile…).
    for item in items {
        harvest_form(source, item, table, replaced);
    }
}

/// A `(put 'SYM 'lisp-indent-function SPEC)` / `(function-put …)` form.
fn harvest_put(
    source: &str,
    head: &str,
    items: &[Datum<'_>],
    table: &mut IndentTable,
    replaced: &mut usize,
) {
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
        insert_spec(source, symbol, spec, table, replaced);
    }
}

/// A `(def… NAME … (declare … (indent SPEC) …) …)` form.
fn harvest_declare(
    source: &str,
    head: &str,
    items: &[Datum<'_>],
    table: &mut IndentTable,
    replaced: &mut usize,
) {
    if !(head.starts_with("def") || head.starts_with("cl-def")) {
        return;
    }
    let Some(name) = items.get(1).and_then(|d| d.as_symbol()) else {
        return;
    };
    // Find a `(declare …)` among the form's items, then an `(indent SPEC)`.
    for item in &items[2.min(items.len())..] {
        let Some(decl) = item.items() else {
            continue;
        };
        if item.head_symbol() != Some("declare") {
            continue;
        }
        for clause in &decl[1..] {
            if clause.head_symbol() == Some("indent") {
                if let Some(spec) = clause.items().and_then(|items| items.get(1)) {
                    insert_spec(source, name, spec, table, replaced);
                }
            }
        }
    }
}

/// Classify and insert an indent spec; an elisp `nil` spec means "no special
/// indent" and yields no entry.
fn insert_spec(
    source: &str,
    symbol: &str,
    spec: &Datum<'_>,
    table: &mut IndentTable,
    replaced: &mut usize,
) {
    if let Some(classified) = indent_spec(source, spec) {
        if table.get(symbol).is_some() {
            *replaced += 1;
        }
        table.insert(symbol, classified);
    }
}

/// Classify an indent spec datum. Unwraps a leading quote (for `put` forms);
/// `nil` yields `None` (no special indent).
fn indent_spec(source: &str, spec: &Datum<'_>) -> Option<IndentSpec> {
    match &spec.kind {
        DatumKind::Number(n) => Some(
            n.parse::<u32>()
                .map(IndentSpec::Number)
                .unwrap_or_else(|_| IndentSpec::Raw(spec.span.text(source).to_owned())),
        ),
        DatumKind::Symbol("nil") => None,
        DatumKind::Symbol("defun") => Some(IndentSpec::Defun),
        DatumKind::Symbol(name) => Some(IndentSpec::Function((*name).to_owned())),
        DatumKind::Prefixed {
            prefix: Prefix::Quote,
            inner,
            ..
        } => indent_spec(source, inner),
        _ => Some(IndentSpec::Raw(spec.span.text(source).to_owned())),
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
        return inner.as_symbol();
    }
    None
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
        assert_eq!(
            table.get("m"),
            Some(&IndentSpec::Function("my-indent-fn".into()))
        );
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
        assert_eq!(table.get("weird"), Some(&IndentSpec::Raw("(1 2)".into())));
    }

    #[test]
    fn nil_spec_yields_no_entry() {
        // `nil` means "no special indent" — not an indent function named nil.
        let table = harvest_indent_specs("(put 'plain 'lisp-indent-function nil)");
        assert!(table.get("plain").is_none());
    }

    #[test]
    fn vector_literal_content_is_not_harvested() {
        // A (put …) inside an elisp `[…]` vector is data, never executed.
        let table = harvest_indent_specs("(defconst k [(put 'y 'lisp-indent-function 5)])");
        assert!(table.get("y").is_none());
    }

    #[test]
    fn quoted_list_content_is_not_harvested() {
        let table = harvest_indent_specs("(defvar d '((put 'z 'lisp-indent-function 5)))");
        assert!(table.get("z").is_none());
    }

    #[test]
    fn table_outlives_and_merges_across_sources() {
        // The owned table accumulates across files and outlives the sources.
        let mut table = IndentTable::new();
        {
            let src_a = String::from("(put 'a 'lisp-indent-function 1)");
            let src_b = String::from("(put 'b 'lisp-indent-function 2)");
            harvest_indent_specs_into(&src_a, &mut table);
            harvest_indent_specs_into(&src_b, &mut table);
            // src_a / src_b dropped here.
        }
        assert_eq!(table.get("a"), Some(&IndentSpec::Number(1)));
        assert_eq!(table.get("b"), Some(&IndentSpec::Number(2)));

        // merge: later table wins on collision.
        let mut other = IndentTable::new();
        other.insert("a", IndentSpec::Defun);
        table.merge(other);
        assert_eq!(table.get("a"), Some(&IndentSpec::Defun));
    }

    #[test]
    fn empty_when_no_specs() {
        let table = harvest_indent_specs("(defun plain (x) x)");
        assert!(table.is_empty());
    }
}
