//! Definition-form annotation (ADR-0019).
//!
//! A best-effort utility layer *over* the reader's [`Datum`] tree —
//! not part of the reader-only core (ADR-0001). It recognizes definition forms
//! (`defun`, `defmacro`, `cl-defun`, `define-minor-mode`, project-local
//! def-macros, …) and tags their parts with [`Role`]s (name, arglist, docstring,
//! body) so a consumer can locate a definition's pieces without hard-coding
//! every macro.
//!
//! Two pieces:
//! - a [`Registry`] of [`FormSpec`]s keyed by head symbol, built from
//!   [`emacs_lisp_builtins`] and extended by the harvester ([`harvest_source`]),
//!   which reads a macro's `declare` metadata and — crucially for third-party
//!   macros — its own arglist parameter names;
//! - the annotator ([`annotate_form`] / [`annotate_tree`]), the dialect-agnostic
//!   mechanism that applies a spec to a form.
//!
//! It never expands or evaluates macros; it only interprets declared/structural
//! metadata.

use std::collections::HashMap;

use crate::datum::{Datum, DatumKind};
use crate::options::Options;
use crate::reader::parse;

/// The role of an argument within a definition form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The leading def-macro symbol itself (e.g. `defun`).
    Keyword,
    /// The defined name.
    Name,
    /// The parameter list.
    Arglist,
    /// The documentation string.
    Docstring,
    /// A `(declare …)` form.
    Declare,
    /// An `(interactive …)` form.
    Interactive,
    /// A body form.
    Body,
    /// A fixed argument whose role could not be classified.
    Other,
}

/// How confidently a [`FormSpec`] was determined — also a coarse provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// A bundled spec for a standard form.
    Builtin,
    /// From an explicit `declare` (`debug (&define …)` or `doc-string N`).
    Declared,
    /// Inferred from the macro's own arglist parameter names.
    Inferred,
    /// From weak signals only (naming conventions).
    Weak,
}

/// A description of how to interpret a definition form's arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct FormSpec {
    /// The head symbol this spec applies to, e.g. `"cl-defun"`.
    pub head: String,
    /// Roles for the fixed leading arguments (right after the head), in order —
    /// e.g. `[Name, Arglist]` for `defun`.
    pub leading: Vec<Role>,
    /// Whether an optional docstring (a string literal) may follow the leading
    /// arguments.
    pub docstring: bool,
    /// Whether the remaining arguments are body forms.
    pub body: bool,
    /// How confidently this spec was determined.
    pub confidence: Confidence,
}

impl FormSpec {
    fn new(
        head: impl Into<String>,
        leading: Vec<Role>,
        docstring: bool,
        body: bool,
        confidence: Confidence,
    ) -> Self {
        FormSpec {
            head: head.into(),
            leading,
            docstring,
            body,
            confidence,
        }
    }
}

/// A set of [`FormSpec`]s keyed by head symbol.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    specs: HashMap<String, FormSpec>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Registry::default()
    }

    /// Insert a spec, overwriting any existing entry for the same head.
    pub fn insert(&mut self, spec: FormSpec) {
        self.specs.insert(spec.head.clone(), spec);
    }

    /// The spec registered for `head`, if any.
    pub fn get(&self, head: &str) -> Option<&FormSpec> {
        self.specs.get(head)
    }

    /// The number of registered specs.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether the registry has no specs.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// One role-tagged child of an annotated form.
#[derive(Debug)]
pub struct Part<'a, 't> {
    /// The role this child plays in the definition form.
    pub role: Role,
    /// The child datum.
    pub datum: &'a Datum<'t>,
}

/// An annotated definition form.
#[derive(Debug)]
pub struct Annotated<'a, 't> {
    /// The head symbol (the def-macro name).
    pub head: &'t str,
    /// The role-tagged children, including the leading `Keyword`.
    pub parts: Vec<Part<'a, 't>>,
    /// Confidence of the spec used.
    pub confidence: Confidence,
}

impl<'a, 't> Annotated<'a, 't> {
    /// The first child with the given role, if any.
    pub fn first(&self, role: Role) -> Option<&'a Datum<'t>> {
        self.parts.iter().find(|p| p.role == role).map(|p| p.datum)
    }
}

/// The head symbol of a round/square list, if the first item is a symbol.
fn list_head<'a, 't>(datum: &'a Datum<'t>) -> Option<(&'t str, &'a [Datum<'t>])> {
    if let DatumKind::List { items, .. } = &datum.kind {
        if let Some(first) = items.first() {
            if let DatumKind::Symbol(s) = first.kind {
                return Some((s, items));
            }
        }
    }
    None
}

/// Annotate a single form if its head is a known definition form.
pub fn annotate_form<'a, 't>(form: &'a Datum<'t>, reg: &Registry) -> Option<Annotated<'a, 't>> {
    let (head, items) = list_head(form)?;
    let spec = reg.get(head)?;

    let mut parts = Vec::with_capacity(items.len());
    parts.push(Part {
        role: Role::Keyword,
        datum: &items[0],
    });

    let mut i = 1;
    for &role in &spec.leading {
        if i >= items.len() {
            break;
        }
        parts.push(Part {
            role,
            datum: &items[i],
        });
        i += 1;
    }

    // Optional docstring: a string that is followed by at least one more form
    // (a lone trailing string is a return value / body, not a docstring).
    if spec.docstring && i < items.len() {
        if let DatumKind::Str(_) = items[i].kind {
            if i + 1 < items.len() {
                parts.push(Part {
                    role: Role::Docstring,
                    datum: &items[i],
                });
                i += 1;
            }
        }
    }

    if spec.body {
        for item in &items[i..] {
            let role = match list_head(item) {
                Some(("declare", _)) => Role::Declare,
                Some(("interactive", _)) => Role::Interactive,
                _ => Role::Body,
            };
            parts.push(Part { role, datum: item });
        }
    }

    Some(Annotated {
        head,
        parts,
        confidence: spec.confidence,
    })
}

/// Recursively annotate every definition form in `data`, in source order
/// (outer forms before the inner forms they contain).
pub fn annotate_tree<'a, 't>(data: &'a [Datum<'t>], reg: &Registry) -> Vec<Annotated<'a, 't>> {
    let mut out = Vec::new();
    for datum in data {
        collect(datum, reg, &mut out);
    }
    out
}

fn collect<'a, 't>(datum: &'a Datum<'t>, reg: &Registry, out: &mut Vec<Annotated<'a, 't>>) {
    if let Some(annotated) = annotate_form(datum, reg) {
        out.push(annotated);
    }
    if let DatumKind::List { items, .. } = &datum.kind {
        for item in items {
            collect(item, reg, out);
        }
    }
}

// --- Harvester (Emacs Lisp) ------------------------------------------------

fn strip_earmuffs(s: &str) -> &str {
    s.trim_matches(|c| c == '*' || c == '_')
}

fn classify_param(name: &str) -> Option<Role> {
    match strip_earmuffs(&name.to_ascii_lowercase()) {
        "name" | "names" | "symbol" | "sym" | "fsym" | "fn-name" | "var" | "variable" | "place"
        | "target" | "def" => Some(Role::Name),
        "arglist" | "args" | "arguments" | "lambda-list" | "key-args" | "params" | "parameters"
        | "ll" => Some(Role::Arglist),
        "docstring" | "doc" | "doc-string" => Some(Role::Docstring),
        "body" | "forms" | "bodyform" | "def-body" | "rest" | "heads" | "clauses" => {
            Some(Role::Body)
        }
        _ => None,
    }
}

/// Harvest definition-form specs from Emacs Lisp `source` into `reg`.
///
/// For each top-level `(defmacro NAME ARGLIST …)` / `(cl-defmacro …)`, derives a
/// [`FormSpec`] from the arglist parameter names plus any `declare` metadata
/// (`doc-string`, `debug (&define …)`). Returns the number of specs added.
pub fn harvest_source(source: &str, reg: &mut Registry) -> usize {
    let parsed = parse(source, &Options::emacs_lisp());
    let mut added = 0;
    for datum in &parsed.data {
        if let Some(spec) = harvest_defmacro(datum) {
            reg.insert(spec);
            added += 1;
        }
    }
    added
}

fn harvest_defmacro(form: &Datum<'_>) -> Option<FormSpec> {
    let (head, items) = list_head(form)?;
    if head != "defmacro" && head != "cl-defmacro" {
        return None;
    }
    // items: [defmacro, NAME, ARGLIST, body...]
    let name = match items.get(1)?.kind {
        DatumKind::Symbol(s) => s,
        _ => return None,
    };
    let DatumKind::List { items: params, .. } = &items.get(2)?.kind else {
        return None;
    };

    let mut leading = Vec::new();
    let mut docstring = false;
    let mut body = false;
    let mut matched_any = false;
    let mut rest = false;

    for p in params {
        let DatumKind::Symbol(pname) = p.kind else {
            continue;
        };
        if pname == "&optional" {
            continue;
        }
        if pname == "&rest" || pname == "&body" {
            rest = true;
            continue;
        }
        match classify_param(pname) {
            Some(Role::Body) => {
                body = true;
                matched_any = true;
                break;
            }
            Some(Role::Docstring) => {
                docstring = true;
                matched_any = true;
            }
            Some(role) => {
                leading.push(role);
                matched_any = true;
                if rest {
                    // a &rest param that classified as name/arglist is unusual;
                    // treat the remainder as body to be safe.
                    body = true;
                    break;
                }
            }
            None => leading.push(Role::Other),
        }
        if rest {
            body = true;
            break;
        }
    }

    // Refine with `declare` metadata in the body.
    let mut declared = false;
    for item in &items[3.min(items.len())..] {
        if let Some(("declare", decl_items)) = list_head(item) {
            for spec in &decl_items[1..] {
                if let Some((key, _)) = list_head(spec) {
                    match key {
                        "doc-string" => {
                            docstring = true;
                            declared = true;
                        }
                        "debug" => declared = true, // often (&define …)
                        _ => {}
                    }
                }
            }
        }
    }

    let confidence = if declared {
        Confidence::Declared
    } else if matched_any {
        Confidence::Inferred
    } else {
        Confidence::Weak
    };

    Some(FormSpec::new(name, leading, docstring, body, confidence))
}

// --- Bundled builtins ------------------------------------------------------

/// A registry of the standard GNU Emacs definition forms.
pub fn emacs_lisp_builtins() -> Registry {
    use Role::{Arglist, Name};
    let mut reg = Registry::new();
    let mut def = |head: &str, leading: Vec<Role>, doc: bool, body: bool| {
        reg.insert(FormSpec::new(head, leading, doc, body, Confidence::Builtin));
    };

    // Function/macro definitions: NAME ARGLIST [DOC] BODY…
    for head in [
        "defun",
        "defmacro",
        "defsubst",
        "cl-defun",
        "cl-defmacro",
        "cl-defsubst",
        "cl-defgeneric",
        "cl-defmethod",
        "define-inline",
    ] {
        def(head, vec![Name, Arglist], true, true);
    }

    // Variable definitions: NAME [VALUE/BODY] [DOC]. Modeled as NAME + body so
    // the value and docstring are captured as body without over-committing.
    for head in [
        "defvar",
        "defvar-local",
        "defconst",
        "defcustom",
        "defface",
        "defgroup",
        "defvar-keymap",
    ] {
        def(head, vec![Name], true, true);
    }

    // Mode/struct-style: NAME [DOC] BODY…
    for head in [
        "define-minor-mode",
        "define-derived-mode",
        "define-global-minor-mode",
        "cl-defstruct",
        "ert-deftest",
    ] {
        def(head, vec![Name], true, true);
    }

    reg
}
