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
//! - a [`Registry`] of [`FormSpec`]s keyed by head symbol: start from the
//!   bundled per-dialect core ([`bundled_registry`], ADR-0020), extend with the
//!   harvester ([`harvest_source`]) — which reads a macro's `declare` metadata
//!   and, crucially for third-party macros, its own arglist parameter names —
//!   and layer consumer-authored specs ([`FormSpec::define`]) on top;
//! - the annotator ([`annotate_form`] / [`annotate_tree`]), the dialect-agnostic
//!   mechanism that applies a spec to a form.
//!
//! It never expands or evaluates macros; it only interprets declared/structural
//! metadata.

use std::collections::HashMap;

use crate::datum::{Datum, DatumKind, Delim};
use crate::options::{Dialect, Options};
use crate::reader::parse;

/// The role of an argument within a definition form.
///
/// `#[non_exhaustive]`: new roles are added as the spec vocabulary grows
/// (ADR-0021 added three), without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    /// The leading def-macro symbol itself (e.g. `defun`).
    Keyword,
    /// The defined name.
    Name,
    /// A method qualifier (`:around`, `:before`, user-defined) appearing between
    /// the name and the arglist (ADR-0021). Variable-length: a method may carry
    /// zero or more. Read as a token only.
    Qualifier,
    /// Clojure `defmethod`'s single arbitrary dispatch datum (e.g. `:circle`),
    /// distinct from a [`Role::Qualifier`] (ADR-0021).
    DispatchValue,
    /// A method's *specialized* arglist (`((x integer))`), whose required
    /// parameters split into `(variable, specializer)` pairs (ADR-0021). Use
    /// [`Annotated::specialized_params`] to decompose it.
    SpecializedArglist,
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

/// An optional, normalized classification hint on a [`FormSpec`] (ADR-0020).
///
/// Attached only where the mapping is uncontested. Ambiguous forms (e.g. Clojure
/// `def`, which may bind a value or a function) carry no category and expose
/// only their raw head symbol (the *Kind*). `#[non_exhaustive]` so new
/// categories can be added without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Category {
    /// A function definition (`defun`, `defn`, CL `defun`).
    Function,
    /// A macro definition (`defmacro`, `define-syntax`).
    Macro,
    /// A variable/binding definition (`defvar`, `defparameter`).
    Variable,
    /// A constant definition (`defconstant`, `defconst`).
    Constant,
    /// A class definition (`defclass`).
    Class,
    /// A struct/record definition (`cl-defstruct`, `defrecord`, `deftype`).
    Struct,
    /// A generic-function definition (`defgeneric`, `defmulti`, `defprotocol`).
    Generic,
    /// A method definition (`defmethod`, `cl-defmethod`).
    Method,
    /// A type definition (`define-record-type`).
    Type,
    /// A test definition (`ert-deftest`, `deftest`).
    Test,
}

/// How confidently a [`FormSpec`] was determined — also a coarse provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Confidence {
    /// A bundled spec for a standard form.
    Builtin,
    /// From an explicit `declare` (`debug (&define …)` or `doc-string N`).
    Declared,
    /// Inferred from the macro's own arglist parameter names.
    Inferred,
    /// From weak signals only (naming conventions).
    Weak,
    /// Supplied directly by the consumer via [`FormSpec::define`] — a distinct
    /// provenance from source-derived specs (authoritative for that consumer).
    Consumer,
}

/// How a dispatch/method form carries its dispatch signature, right after the
/// name (ADR-0021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Dispatch {
    /// CL/elisp/ISLisp style: zero or more [`Role::Qualifier`]s greedily consumed
    /// up to the first delimited list, which is the [`Role::SpecializedArglist`].
    Qualifiers,
    /// Clojure style: exactly one [`Role::DispatchValue`] datum, then a square
    /// [`Role::Arglist`] vector.
    Value,
}

/// Where a form's optional docstring sits, and when a string counts as one.
///
/// The one-size `bool` this replaces mis-tagged the whole `defvar` family
/// (value-before-doc) and missed elisp's lone-string docstrings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Docstring {
    /// The form has no positional docstring.
    None,
    /// A string right after the leading roles, only when at least one more
    /// form follows it — a lone trailing string is a value, not documentation
    /// (Common Lisp rule, CLHS 3.4.11).
    Leading,
    /// Like [`Docstring::Leading`], but a string that is the *only* form after
    /// the leading roles also counts (Emacs Lisp: `(defun f () "doc")` is
    /// documented; likewise Python-style dialects such as Hy).
    LeadingOrLone,
}

/// A description of how to interpret a definition form's arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct FormSpec {
    /// The head symbol this spec applies to, e.g. `"cl-defun"`.
    pub head: String,
    /// Roles for the fixed leading arguments (right after the head), in order —
    /// e.g. `[Name, Arglist]` for `defun`.
    pub leading: Vec<Role>,
    /// Where the optional docstring sits, if the form has one.
    pub docstring: Docstring,
    /// Whether the remaining arguments are body forms.
    pub body: bool,
    /// An optional normalized category hint (ADR-0020), set only where the
    /// mapping is uncontested. The verbatim head symbol ([`Self::head`], the
    /// *Kind*) is always the faithful classification.
    pub category: Option<Category>,
    /// For dispatch/method forms, how the dispatch signature after the name is
    /// read (ADR-0021). `None` for ordinary def-forms.
    pub dispatch: Option<Dispatch>,
    /// How confidently this spec was determined.
    pub confidence: Confidence,
}

impl FormSpec {
    fn new(
        head: impl Into<String>,
        leading: Vec<Role>,
        docstring: Docstring,
        body: bool,
        confidence: Confidence,
    ) -> Self {
        FormSpec {
            head: head.into(),
            leading,
            docstring,
            body,
            category: None,
            dispatch: None,
            confidence,
        }
    }

    /// Build a consumer-supplied definition-form spec (ADR-0020).
    ///
    /// Consumers compose these on top of [`bundled_registry`] to cover their
    /// project-local def-macros. The confidence is recorded as
    /// [`Confidence::Consumer`]; chain [`Self::with_category`] to add a hint.
    pub fn define(
        head: impl Into<String>,
        leading: Vec<Role>,
        docstring: Docstring,
        body: bool,
    ) -> Self {
        FormSpec::new(head, leading, docstring, body, Confidence::Consumer)
    }

    /// Attach a normalized [`Category`] hint (builder style).
    pub fn with_category(mut self, category: Category) -> Self {
        self.category = Some(category);
        self
    }

    /// Mark this as a dispatch/method form with the given [`Dispatch`] shape
    /// (builder style, ADR-0021).
    pub fn with_dispatch(mut self, dispatch: Dispatch) -> Self {
        self.dispatch = Some(dispatch);
        self
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

    /// Remove and return the spec registered for `head`, if any.
    pub fn remove(&mut self, head: &str) -> Option<FormSpec> {
        self.specs.remove(head)
    }

    /// Iterate the registered specs (no defined order).
    pub fn iter(&self) -> impl Iterator<Item = &FormSpec> {
        self.specs.values()
    }

    /// Merge `other` into `self`; on a head collision, `other`'s spec wins.
    /// This is the composition step ADR-0020 describes: bundled core, then
    /// harvested specs, then the consumer's own registry, later layers winning.
    pub fn merge(&mut self, other: Registry) {
        self.specs.extend(other.specs);
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

impl Extend<FormSpec> for Registry {
    fn extend<T: IntoIterator<Item = FormSpec>>(&mut self, iter: T) {
        for spec in iter {
            self.insert(spec);
        }
    }
}

impl FromIterator<FormSpec> for Registry {
    fn from_iter<T: IntoIterator<Item = FormSpec>>(iter: T) -> Self {
        let mut reg = Registry::new();
        reg.extend(iter);
        reg
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
    /// The annotated form itself — its span is the definition's full extent.
    pub form: &'a Datum<'t>,
    /// The head symbol (the def-macro name) — the verbatim *Kind* (ADR-0020),
    /// always faithful.
    pub head: &'t str,
    /// The role-tagged children, including the leading `Keyword`.
    pub parts: Vec<Part<'a, 't>>,
    /// The normalized [`Category`] hint of the spec used, if any (ADR-0020).
    pub category: Option<Category>,
    /// Confidence of the spec used.
    pub confidence: Confidence,
}

impl<'a, 't> Annotated<'a, 't> {
    /// The first child with the given role, if any.
    pub fn first(&self, role: Role) -> Option<&'a Datum<'t>> {
        self.parts.iter().find(|p| p.role == role).map(|p| p.datum)
    }

    /// Every child with the given role, in order.
    pub fn all(&self, role: Role) -> impl Iterator<Item = &'a Datum<'t>> + '_ {
        self.parts
            .iter()
            .filter(move |p| p.role == role)
            .map(|p| p.datum)
    }

    /// The required parameters of this form's [`Role::SpecializedArglist`], each
    /// split into a `(variable, specializer)` pair (ADR-0021). Empty if the form
    /// has no specialized arglist. Specializers are verbatim Datums — lispexp
    /// neither resolves types nor evaluates an `(eql form)`.
    pub fn specialized_params(&self) -> Vec<SpecializedParam<'a, 't>> {
        match self.first(Role::SpecializedArglist) {
            Some(arglist) => split_specialized_arglist(arglist),
            None => Vec::new(),
        }
    }
}

/// A required parameter of a specialized arglist, split into its variable token
/// and optional specializer (ADR-0021).
#[derive(Debug)]
pub struct SpecializedParam<'a, 't> {
    /// The parameter variable — a symbol Datum such as `x`.
    pub variable: &'a Datum<'t>,
    /// The specializer Datum, if the parameter was specialized: a symbol Datum
    /// for `(x integer)`, a list Datum for `(x (eql form))`. `None` for an
    /// unspecialized parameter written as a bare symbol.
    pub specializer: Option<&'a Datum<'t>>,
}

/// Split a specialized arglist's required parameters into `(variable,
/// specializer)` pairs (ADR-0021). Stops at the first lambda-list marker
/// (`&optional`, `&rest`, `&key`, …); those tail parameters are not specialized.
pub fn split_specialized_arglist<'a, 't>(arglist: &'a Datum<'t>) -> Vec<SpecializedParam<'a, 't>> {
    let DatumKind::List { items, .. } = &arglist.kind else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        match &item.kind {
            // A lambda-list marker ends the required section.
            DatumKind::Symbol(s) if s.starts_with('&') => break,
            // A bare symbol is an unspecialized parameter.
            DatumKind::Symbol(_) => out.push(SpecializedParam {
                variable: item,
                specializer: None,
            }),
            // `(var specializer)` splits into the pair; the specializer is the
            // whole second element, verbatim.
            DatumKind::List { items: pair, .. } => {
                if let Some(var) = pair.first() {
                    out.push(SpecializedParam {
                        variable: var,
                        specializer: pair.get(1),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Whether a datum is a delimited list of any shape (`()`/`[]`/`{}`/`#{}`) —
/// the arglist boundary for a qualifier run (ADR-0021). Shape only.
fn is_delimited_list(datum: &Datum<'_>) -> bool {
    matches!(datum.kind, DatumKind::List { .. })
}

/// The head symbol of a round/square list, if the first item is a symbol.
fn list_head<'a, 't>(datum: &'a Datum<'t>) -> Option<(&'t str, &'a [Datum<'t>])> {
    let items = datum.items()?;
    let head = datum.head_symbol()?;
    Some((head, items))
}

/// Annotate a single form if its head is a known definition form.
#[must_use]
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
        // Shape guard: a defined Name must be a symbol or a `(setf foo)`-style
        // round list. Anything else (e.g. Fennel's anonymous `(fn [x] x)`,
        // where `[x]` would land in the Name slot) means this instance is not
        // a named definition — don't annotate it.
        if role == Role::Name && !is_name_shaped(&items[i]) {
            return None;
        }
        parts.push(Part {
            role,
            datum: &items[i],
        });
        i += 1;
    }

    // Dispatch signature (ADR-0021), read right after the name.
    match spec.dispatch {
        Some(Dispatch::Qualifiers) => {
            // Greedily consume qualifiers up to the first delimited list, which
            // is the specialized arglist. Boundary uses token shape only —
            // sound per CLHS `defmethod`, whose qualifiers are non-lists.
            while i < items.len() && !is_delimited_list(&items[i]) {
                parts.push(Part {
                    role: Role::Qualifier,
                    datum: &items[i],
                });
                i += 1;
            }
            if i < items.len() && is_delimited_list(&items[i]) {
                parts.push(Part {
                    role: Role::SpecializedArglist,
                    datum: &items[i],
                });
                i += 1;
            }
        }
        Some(Dispatch::Value) => {
            // Exactly one dispatch datum, then a square arglist vector.
            // Requiring Square distinguishes the arglist from a multi-arity
            // clause `([a] …)`, which is a round list.
            if i < items.len() {
                parts.push(Part {
                    role: Role::DispatchValue,
                    datum: &items[i],
                });
                i += 1;
            }
            if i < items.len()
                && matches!(
                    items[i].kind,
                    DatumKind::List {
                        delim: Delim::Square,
                        ..
                    }
                )
            {
                parts.push(Part {
                    role: Role::Arglist,
                    datum: &items[i],
                });
                i += 1;
            }
        }
        None => {}
    }

    // Optional docstring, per the spec's placement policy.
    if i < items.len() {
        let is_str = matches!(items[i].kind, DatumKind::Str(_));
        let accepted = match spec.docstring {
            Docstring::None => false,
            // A lone trailing string is a value, not documentation (CL rule).
            Docstring::Leading => is_str && i + 1 < items.len(),
            // Emacs Lisp / Python-style: a lone body string is the docstring.
            Docstring::LeadingOrLone => is_str,
        };
        if accepted {
            parts.push(Part {
                role: Role::Docstring,
                datum: &items[i],
            });
            i += 1;
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
        form,
        head,
        parts,
        category: spec.category,
        confidence: spec.confidence,
    })
}

/// Whether a datum can be a defined name: a symbol, or a round list whose head
/// is a symbol (a CL `(setf foo)` function name). Rejects vectors/maps, so an
/// arglist sliding into the Name slot (anonymous forms) is caught by shape.
fn is_name_shaped(datum: &Datum<'_>) -> bool {
    match &datum.kind {
        DatumKind::Symbol(_) => true,
        DatumKind::List {
            delim: Delim::Round,
            items,
            ..
        } => matches!(items.first().map(|d| &d.kind), Some(DatumKind::Symbol(_))),
        _ => false,
    }
}

/// Recursively annotate every definition form in `data`, in source order
/// (outer forms before the inner forms they contain).
#[must_use]
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
        if rest {
            // The &rest param stands for the remainder, never one fixed slot —
            // whatever its name classifies as, the remainder is the body.
            if classify_param(pname).is_some() {
                matched_any = true;
            }
            body = true;
            break;
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
            }
            None => leading.push(Role::Other),
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

    // Harvested macros are elisp, where a lone body string is a docstring.
    let docstring = if docstring {
        Docstring::LeadingOrLone
    } else {
        Docstring::None
    };
    Some(FormSpec::new(name, leading, docstring, body, confidence))
}

// --- Bundled builtins ------------------------------------------------------

/// A small builder for a bundled per-dialect registry: each entry is a
/// `Confidence::Builtin` [`FormSpec`] with an optional [`Category`] hint.
struct Builtins {
    reg: Registry,
}

impl Builtins {
    fn new() -> Self {
        Builtins {
            reg: Registry::new(),
        }
    }

    /// Register `head` with the given leading roles, docstring policy, body
    /// flag, and optional category hint.
    fn def(
        &mut self,
        head: &str,
        leading: Vec<Role>,
        doc: Docstring,
        body: bool,
        category: Option<Category>,
    ) {
        let mut spec = FormSpec::new(head, leading, doc, body, Confidence::Builtin);
        spec.category = category;
        self.reg.insert(spec);
    }

    /// Register a dispatch/method form (ADR-0021): NAME, a dispatch signature,
    /// an optional docstring after the arglist, then body.
    fn method(&mut self, head: &str, dispatch: Dispatch, doc: Docstring, category: Category) {
        let mut spec = FormSpec::new(head, vec![Role::Name], doc, true, Confidence::Builtin);
        spec.category = Some(category);
        spec.dispatch = Some(dispatch);
        self.reg.insert(spec);
    }
}

/// The bundled conservative core [`Registry`] for `dialect` (ADR-0020).
///
/// Only the high-confidence, uncontested def-forms are included; a project's
/// long tail (project-local def-macros, contested classifications) stays with
/// the consumer, composed on top of this core. EDN, a data-only dialect, has no
/// definitions and returns an empty registry.
#[must_use]
pub fn bundled_registry(dialect: Dialect) -> Registry {
    match dialect {
        Dialect::Scheme
        | Dialect::Guile
        | Dialect::Gauche
        | Dialect::Mosh
        | Dialect::Gambit
        | Dialect::SchemeSuperset => scheme_builtins(),
        Dialect::Racket => racket_builtins(),
        Dialect::CommonLisp => common_lisp_builtins(),
        Dialect::EmacsLisp => emacs_lisp_builtins(),
        Dialect::Clojure => clojure_builtins(),
        Dialect::Phel => clojure_builtins(),
        Dialect::Fennel => fennel_builtins(),
        Dialect::Janet => janet_builtins(),
        Dialect::Hy => hy_builtins(),
        Dialect::Lfe => lfe_builtins(),
        Dialect::Islisp => islisp_builtins(),
        Dialect::AutoLisp => autolisp_builtins(),
        Dialect::Edn => Registry::new(),
    }
}

/// The standard GNU Emacs definition forms. An elisp lone body string is a
/// docstring (`(defun f () "doc")` is documented), hence `LeadingOrLone`.
fn emacs_lisp_builtins() -> Registry {
    use Category::{Constant, Function, Generic, Macro, Method, Struct, Test, Variable};
    use Docstring::{LeadingOrLone, None as NoDoc};
    use Role::{Arglist, Name, Other};
    let mut b = Builtins::new();

    // Function/macro definitions: NAME ARGLIST [DOC] BODY… (docstring follows
    // the arglist in elisp).
    let fnlike = vec![Name, Arglist];
    b.def("defun", fnlike.clone(), LeadingOrLone, true, Some(Function));
    b.def(
        "defsubst",
        fnlike.clone(),
        LeadingOrLone,
        true,
        Some(Function),
    );
    b.def(
        "cl-defun",
        fnlike.clone(),
        LeadingOrLone,
        true,
        Some(Function),
    );
    b.def(
        "cl-defsubst",
        fnlike.clone(),
        LeadingOrLone,
        true,
        Some(Function),
    );
    b.def(
        "define-inline",
        fnlike.clone(),
        LeadingOrLone,
        true,
        Some(Function),
    );
    b.def("defmacro", fnlike.clone(), LeadingOrLone, true, Some(Macro));
    b.def(
        "cl-defmacro",
        fnlike.clone(),
        LeadingOrLone,
        true,
        Some(Macro),
    );
    b.def("cl-defgeneric", fnlike, LeadingOrLone, true, Some(Generic));
    b.method("cl-defmethod", Dispatch::Qualifiers, LeadingOrLone, Method);

    // Variable definitions: NAME [VALUE] [DOC]. The value occupies a fixed
    // `Other` slot so the trailing docstring is found after it; with only one
    // argument, the `Other` slot swallows it (a value, not documentation).
    let varlike = vec![Name, Other];
    b.def(
        "defvar",
        varlike.clone(),
        LeadingOrLone,
        false,
        Some(Variable),
    );
    b.def(
        "defvar-local",
        varlike.clone(),
        LeadingOrLone,
        false,
        Some(Variable),
    );
    b.def(
        "defconst",
        varlike.clone(),
        LeadingOrLone,
        false,
        Some(Constant),
    );
    // NAME STANDARD/SPEC/MEMBERS DOC [KEYWORD ARGS]…
    b.def(
        "defcustom",
        varlike.clone(),
        LeadingOrLone,
        true,
        Some(Variable),
    );
    b.def(
        "defface",
        varlike.clone(),
        LeadingOrLone,
        true,
        Some(Variable),
    );
    b.def("defgroup", varlike, LeadingOrLone, true, None);
    // defvar-keymap documents via the `:doc` keyword, not a positional string.
    b.def("defvar-keymap", vec![Name], NoDoc, true, Some(Variable));

    // Mode/struct/test forms.
    b.def("define-minor-mode", vec![Name], LeadingOrLone, true, None);
    // CHILD PARENT NAME [DOC] BODY…
    b.def(
        "define-derived-mode",
        vec![Name, Other, Other],
        LeadingOrLone,
        true,
        None,
    );
    b.def(
        "define-global-minor-mode",
        vec![Name],
        LeadingOrLone,
        true,
        None,
    );
    b.def(
        "cl-defstruct",
        vec![Name],
        LeadingOrLone,
        true,
        Some(Struct),
    );
    // NAME () [DOC] BODY… — the empty arglist is mandatory.
    b.def(
        "ert-deftest",
        vec![Name, Arglist],
        LeadingOrLone,
        true,
        Some(Test),
    );

    b.reg
}

/// Scheme's core definition forms (R7RS). Scheme has no docstrings.
fn scheme_builtins() -> Registry {
    use Category::{Macro, Type};
    use Docstring::None as NoDoc;
    use Role::Name;
    let mut b = Builtins::new();
    // `define` is ambiguous (value vs. procedure) — no category.
    b.def("define", vec![Name], NoDoc, true, None);
    b.def("define-values", vec![Name], NoDoc, true, None);
    b.def("define-syntax", vec![Name], NoDoc, true, Some(Macro));
    b.def("define-record-type", vec![Name], NoDoc, true, Some(Type));
    b.reg
}

/// Racket's core: Scheme's plus `struct`.
fn racket_builtins() -> Registry {
    let mut reg = scheme_builtins();
    reg.insert(
        FormSpec::new(
            "struct",
            vec![Role::Name],
            Docstring::None,
            true,
            Confidence::Builtin,
        )
        .with_category(Category::Struct),
    );
    reg
}

/// Common Lisp's core definition forms (ANSI). A lone trailing string is a
/// value, not documentation (CLHS 3.4.11), hence `Leading` throughout;
/// `defclass`/`defgeneric`/`define-condition` document via a
/// `(:documentation …)` option, not a positional string.
fn common_lisp_builtins() -> Registry {
    use Category::{Class, Constant, Function, Generic, Macro, Method, Struct, Variable};
    use Docstring::{Leading, LeadingOrLone, None as NoDoc};
    use Role::{Arglist, Name, Other};
    let mut b = Builtins::new();
    b.def("defun", vec![Name, Arglist], Leading, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], Leading, true, Some(Macro));
    b.def(
        "defgeneric",
        vec![Name, Arglist],
        NoDoc,
        true,
        Some(Generic),
    );
    b.method("defmethod", Dispatch::Qualifiers, Leading, Method);
    // NAME [VALUE [DOC]] — no body; the doc is last, after the value slot.
    // defvar's value is optional, so a lone string after NAME is the value
    // (swallowed by the Other slot), and a string after the value is the doc.
    b.def(
        "defvar",
        vec![Name, Other],
        LeadingOrLone,
        false,
        Some(Variable),
    );
    b.def(
        "defparameter",
        vec![Name, Other],
        LeadingOrLone,
        false,
        Some(Variable),
    );
    b.def(
        "defconstant",
        vec![Name, Other],
        LeadingOrLone,
        false,
        Some(Constant),
    );
    b.def("defclass", vec![Name], NoDoc, true, Some(Class));
    b.def("define-condition", vec![Name], NoDoc, true, Some(Class));
    // NAME-AND-OPTIONS [DOC] SLOTS… (CLHS defstruct allows a docstring).
    b.def("defstruct", vec![Name], Leading, true, Some(Struct));
    b.def("defpackage", vec![Name], NoDoc, true, None);
    b.reg
}

/// Clojure's core definition forms (also used for Phel). A docstring precedes
/// the arglist and is always followed by it, so `Leading` works; leading is
/// NAME only (no reliable fixed-position Arglist).
fn clojure_builtins() -> Registry {
    use Category::{Function, Generic, Macro, Method, Struct, Test, Type};
    use Docstring::{Leading, None as NoDoc};
    use Role::Name;
    let mut b = Builtins::new();
    b.def("defn", vec![Name], Leading, true, Some(Function));
    b.def("defn-", vec![Name], Leading, true, Some(Function));
    b.def("defmacro", vec![Name], Leading, true, Some(Macro));
    // `(def x "str")` is a string *value*; only `(def x "doc" init)` has a doc —
    // exactly the Leading rule.
    b.def("def", vec![Name], Leading, true, None); // value vs. fn — ambiguous
    b.def("defonce", vec![Name], NoDoc, true, None);
    b.def("defmulti", vec![Name], Leading, true, Some(Generic));
    b.method("defmethod", Dispatch::Value, NoDoc, Method);
    b.def("defprotocol", vec![Name], Leading, true, Some(Generic));
    b.def("definterface", vec![Name], NoDoc, true, Some(Type));
    b.def("defrecord", vec![Name], NoDoc, true, Some(Struct));
    // `deftype` builds a bare host type — arguably Struct, but contested;
    // Kind-only per ADR-0020.
    b.def("deftype", vec![Name], NoDoc, true, None);
    b.def("deftest", vec![Name], NoDoc, true, Some(Test));
    // NAME [DOC] [ATTR-MAP] REFERENCES… — ubiquitous and unambiguous.
    b.def("ns", vec![Name], Leading, true, None);
    b.reg
}

/// Fennel's core definition forms. `fn`/`lambda` may be anonymous —
/// `(fn [x] …)` — which the annotator's Name shape guard rejects, so only the
/// named form annotates.
fn fennel_builtins() -> Registry {
    use Category::{Function, Macro, Variable};
    use Docstring::{Leading, None as NoDoc};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    // Fennel's docstring follows the arglist.
    b.def("fn", vec![Name, Arglist], Leading, true, Some(Function));
    b.def("lambda", vec![Name, Arglist], Leading, true, Some(Function));
    b.def("λ", vec![Name, Arglist], Leading, true, Some(Function));
    b.def("macro", vec![Name, Arglist], Leading, true, Some(Macro));
    b.def("macros", vec![Name], NoDoc, true, Some(Macro));
    b.def("local", vec![Name], NoDoc, true, Some(Variable));
    b.def("var", vec![Name], NoDoc, true, Some(Variable));
    b.def("global", vec![Name], NoDoc, true, Some(Variable));
    b.reg
}

/// Janet's core definition forms. The docstring precedes the arglist/value —
/// leading is NAME only, and a lone string after the name is a value.
fn janet_builtins() -> Registry {
    use Category::{Function, Macro, Variable};
    use Docstring::Leading;
    use Role::Name;
    let mut b = Builtins::new();
    b.def("defn", vec![Name], Leading, true, Some(Function));
    b.def("defn-", vec![Name], Leading, true, Some(Function));
    b.def("defmacro", vec![Name], Leading, true, Some(Macro));
    b.def("defmacro-", vec![Name], Leading, true, Some(Macro));
    b.def("def", vec![Name], Leading, true, None);
    b.def("def-", vec![Name], Leading, true, None);
    b.def("var", vec![Name], Leading, true, Some(Variable));
    b.def("var-", vec![Name], Leading, true, Some(Variable));
    b.reg
}

/// Hy's core definition forms. Like Python, a lone body string is the
/// docstring.
fn hy_builtins() -> Registry {
    use Category::{Class, Function, Macro};
    use Docstring::{LeadingOrLone, None as NoDoc};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    // Decorated forms `(defn [dec] name [args] …)` put a vector in the Name
    // slot; the shape guard skips them rather than mis-tagging.
    b.def(
        "defn",
        vec![Name, Arglist],
        LeadingOrLone,
        true,
        Some(Function),
    );
    b.def(
        "defmacro",
        vec![Name, Arglist],
        LeadingOrLone,
        true,
        Some(Macro),
    );
    b.def("defclass", vec![Name], LeadingOrLone, true, Some(Class));
    // General assignment, not only a top-level definition — Kind-only.
    b.def("setv", vec![Name], NoDoc, true, None);
    b.reg
}

/// LFE's core definition forms. Note: the pattern-clause form
/// `(defun name ((pat) body) …)` tags its first clause as Arglist — a known
/// shape ambiguity accepted for the conservative core.
fn lfe_builtins() -> Registry {
    use Category::{Function, Macro, Struct};
    use Docstring::{Leading, None as NoDoc};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def("defun", vec![Name, Arglist], Leading, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], Leading, true, Some(Macro));
    b.def("defrecord", vec![Name], NoDoc, true, Some(Struct));
    b.def("defmodule", vec![Name], NoDoc, true, None);
    b.reg
}

/// ISLisp's core definition forms. ISLisp has no docstrings.
fn islisp_builtins() -> Registry {
    use Category::{Class, Constant, Function, Generic, Macro, Method, Variable};
    use Docstring::None as NoDoc;
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def("defun", vec![Name, Arglist], NoDoc, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], NoDoc, true, Some(Macro));
    b.def(
        "defgeneric",
        vec![Name, Arglist],
        NoDoc,
        true,
        Some(Generic),
    );
    b.method("defmethod", Dispatch::Qualifiers, NoDoc, Method);
    b.def("defclass", vec![Name], NoDoc, true, Some(Class));
    b.def("defconstant", vec![Name], NoDoc, true, Some(Constant));
    b.def("defglobal", vec![Name], NoDoc, true, Some(Variable));
    b.def("defdynamic", vec![Name], NoDoc, true, Some(Variable));
    b.reg
}

/// AutoLISP's core definition forms.
fn autolisp_builtins() -> Registry {
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def(
        "defun",
        vec![Name, Arglist],
        Docstring::None,
        true,
        Some(Category::Function),
    );
    b.reg
}
