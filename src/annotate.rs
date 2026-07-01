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
use crate::options::{Dialect, Options};
use crate::reader::parse;

/// The role of an argument within a definition form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// How a dispatch/method form carries its dispatch signature, right after the
/// name (ADR-0021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dispatch {
    /// CL/elisp/ISLisp style: zero or more [`Role::Qualifier`]s greedily consumed
    /// up to the first delimited list, which is the [`Role::SpecializedArglist`].
    Qualifiers,
    /// Clojure style: exactly one [`Role::DispatchValue`] datum, then a plain
    /// [`Role::Arglist`].
    Value,
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
        docstring: bool,
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
    /// [`Confidence::Declared`]; chain [`Self::with_category`] to add a hint.
    pub fn define(
        head: impl Into<String>,
        leading: Vec<Role>,
        docstring: bool,
        body: bool,
    ) -> Self {
        FormSpec::new(head, leading, docstring, body, Confidence::Declared)
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

    // Dispatch signature (ADR-0021), read right after the name.
    match spec.dispatch {
        Some(Dispatch::Qualifiers) => {
            // Greedily consume qualifiers up to the first delimited list, which
            // is the specialized arglist. Boundary uses token shape only.
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
            // Exactly one dispatch datum, then a plain arglist.
            if i < items.len() {
                parts.push(Part {
                    role: Role::DispatchValue,
                    datum: &items[i],
                });
                i += 1;
            }
            if i < items.len() && is_delimited_list(&items[i]) {
                parts.push(Part {
                    role: Role::Arglist,
                    datum: &items[i],
                });
                i += 1;
            }
        }
        None => {}
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
        category: spec.category,
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

    /// Register `head` with the given leading roles, docstring/body flags, and
    /// optional category hint.
    fn def(
        &mut self,
        head: &str,
        leading: Vec<Role>,
        doc: bool,
        body: bool,
        category: Option<Category>,
    ) {
        let mut spec = FormSpec::new(head, leading, doc, body, Confidence::Builtin);
        spec.category = category;
        self.reg.insert(spec);
    }

    /// Register a dispatch/method form (ADR-0021): NAME, a dispatch signature,
    /// then body.
    fn method(&mut self, head: &str, dispatch: Dispatch, category: Category) {
        let mut spec = FormSpec::new(head, vec![Role::Name], false, true, Confidence::Builtin);
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
pub fn bundled_registry(dialect: Dialect) -> Registry {
    match dialect {
        Dialect::Scheme => scheme_builtins(),
        Dialect::SchemeSuperset => scheme_builtins(),
        Dialect::Guile => scheme_builtins(),
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

/// A registry of the standard GNU Emacs definition forms.
pub fn emacs_lisp_builtins() -> Registry {
    use Category::{Constant, Function, Generic, Macro, Method, Struct, Test, Variable};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();

    // Function/macro definitions: NAME ARGLIST [DOC] BODY… (docstring follows
    // the arglist in elisp).
    let fnlike = vec![Name, Arglist];
    b.def("defun", fnlike.clone(), true, true, Some(Function));
    b.def("defsubst", fnlike.clone(), true, true, Some(Function));
    b.def("cl-defun", fnlike.clone(), true, true, Some(Function));
    b.def("cl-defsubst", fnlike.clone(), true, true, Some(Function));
    b.def("define-inline", fnlike.clone(), true, true, Some(Function));
    b.def("defmacro", fnlike.clone(), true, true, Some(Macro));
    b.def("cl-defmacro", fnlike.clone(), true, true, Some(Macro));
    b.def("cl-defgeneric", fnlike, true, true, Some(Generic));
    b.method("cl-defmethod", Dispatch::Qualifiers, Method);

    // Variable definitions: NAME [VALUE/BODY] [DOC], modeled as NAME + body.
    b.def("defvar", vec![Name], true, true, Some(Variable));
    b.def("defvar-local", vec![Name], true, true, Some(Variable));
    b.def("defvar-keymap", vec![Name], true, true, Some(Variable));
    b.def("defcustom", vec![Name], true, true, Some(Variable));
    b.def("defface", vec![Name], true, true, Some(Variable));
    b.def("defconst", vec![Name], true, true, Some(Constant));
    b.def("defgroup", vec![Name], true, true, None);

    // Mode/struct/test forms: NAME [DOC] BODY…
    b.def("define-minor-mode", vec![Name], true, true, None);
    b.def("define-derived-mode", vec![Name], true, true, None);
    b.def("define-global-minor-mode", vec![Name], true, true, None);
    b.def("cl-defstruct", vec![Name], true, true, Some(Struct));
    b.def("ert-deftest", vec![Name], true, true, Some(Test));

    b.reg
}

/// Scheme's core definition forms (R7RS).
pub fn scheme_builtins() -> Registry {
    use Category::{Macro, Type};
    use Role::Name;
    let mut b = Builtins::new();
    // `define` is ambiguous (value vs. procedure) — no category.
    b.def("define", vec![Name], false, true, None);
    b.def("define-values", vec![Name], false, true, None);
    b.def("define-syntax", vec![Name], false, true, Some(Macro));
    b.def("define-record-type", vec![Name], false, true, Some(Type));
    b.reg
}

/// Racket's core: Scheme's plus `struct`.
pub fn racket_builtins() -> Registry {
    let mut reg = scheme_builtins();
    reg.insert(
        FormSpec::new("struct", vec![Role::Name], false, true, Confidence::Builtin)
            .with_category(Category::Struct),
    );
    reg
}

/// Common Lisp's core definition forms (ANSI).
pub fn common_lisp_builtins() -> Registry {
    use Category::{Class, Constant, Function, Generic, Macro, Method, Struct, Variable};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def("defun", vec![Name, Arglist], true, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], true, true, Some(Macro));
    b.def("defgeneric", vec![Name, Arglist], true, true, Some(Generic));
    b.method("defmethod", Dispatch::Qualifiers, Method);
    b.def("defvar", vec![Name], true, true, Some(Variable));
    b.def("defparameter", vec![Name], true, true, Some(Variable));
    b.def("defconstant", vec![Name], true, true, Some(Constant));
    b.def("defclass", vec![Name], true, true, Some(Class));
    b.def("define-condition", vec![Name], true, true, Some(Class));
    b.def("defstruct", vec![Name], true, true, Some(Struct));
    b.def("defpackage", vec![Name], false, true, None);
    b.reg
}

/// Clojure's core definition forms (also used for Phel).
pub fn clojure_builtins() -> Registry {
    use Category::{Function, Generic, Macro, Method, Struct, Test, Type};
    use Role::Name;
    let mut b = Builtins::new();
    // In Clojure a docstring precedes the arglist, so leading is NAME only and
    // the arglist falls into the body (no reliable fixed-position Arglist).
    b.def("defn", vec![Name], true, true, Some(Function));
    b.def("defn-", vec![Name], true, true, Some(Function));
    b.def("defmacro", vec![Name], true, true, Some(Macro));
    b.def("def", vec![Name], false, true, None); // value vs. fn — ambiguous
    b.def("defonce", vec![Name], false, true, None);
    b.def("defmulti", vec![Name], false, true, Some(Generic));
    b.method("defmethod", Dispatch::Value, Method);
    b.def("defprotocol", vec![Name], false, true, Some(Generic));
    b.def("definterface", vec![Name], false, true, Some(Type));
    b.def("defrecord", vec![Name], false, true, Some(Struct));
    b.def("deftype", vec![Name], false, true, Some(Struct));
    b.def("deftest", vec![Name], false, true, Some(Test));
    b.reg
}

/// Fennel's core definition forms.
pub fn fennel_builtins() -> Registry {
    use Category::{Function, Macro, Variable};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    // Fennel's docstring follows the arglist, so NAME ARGLIST works.
    b.def("fn", vec![Name, Arglist], true, true, Some(Function));
    b.def("lambda", vec![Name, Arglist], true, true, Some(Function));
    b.def("λ", vec![Name, Arglist], true, true, Some(Function));
    b.def("macro", vec![Name, Arglist], false, true, Some(Macro));
    b.def("macros", vec![Name], false, true, Some(Macro));
    b.def("local", vec![Name], false, true, Some(Variable));
    b.def("var", vec![Name], false, true, Some(Variable));
    b.def("global", vec![Name], false, true, Some(Variable));
    b.reg
}

/// Janet's core definition forms.
pub fn janet_builtins() -> Registry {
    use Category::{Function, Macro, Variable};
    use Role::Name;
    let mut b = Builtins::new();
    // Janet's docstring precedes the arglist — leading is NAME only.
    b.def("defn", vec![Name], true, true, Some(Function));
    b.def("defn-", vec![Name], true, true, Some(Function));
    b.def("defmacro", vec![Name], true, true, Some(Macro));
    b.def("defmacro-", vec![Name], true, true, Some(Macro));
    b.def("def", vec![Name], true, true, None);
    b.def("def-", vec![Name], true, true, None);
    b.def("var", vec![Name], true, true, Some(Variable));
    b.def("var-", vec![Name], true, true, Some(Variable));
    b.reg
}

/// Hy's core definition forms.
pub fn hy_builtins() -> Registry {
    use Category::{Class, Function, Macro};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def("defn", vec![Name, Arglist], true, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], true, true, Some(Macro));
    b.def("defclass", vec![Name], false, true, Some(Class));
    b.def("setv", vec![Name], false, true, None);
    b.reg
}

/// LFE's core definition forms.
pub fn lfe_builtins() -> Registry {
    use Category::{Function, Macro, Struct};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def("defun", vec![Name, Arglist], true, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], true, true, Some(Macro));
    b.def("defrecord", vec![Name], false, true, Some(Struct));
    b.def("defmodule", vec![Name], false, true, None);
    b.reg
}

/// ISLisp's core definition forms.
pub fn islisp_builtins() -> Registry {
    use Category::{Class, Constant, Function, Generic, Macro, Method, Variable};
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def("defun", vec![Name, Arglist], false, true, Some(Function));
    b.def("defmacro", vec![Name, Arglist], false, true, Some(Macro));
    b.def(
        "defgeneric",
        vec![Name, Arglist],
        false,
        true,
        Some(Generic),
    );
    b.method("defmethod", Dispatch::Qualifiers, Method);
    b.def("defclass", vec![Name], false, true, Some(Class));
    b.def("defconstant", vec![Name], false, true, Some(Constant));
    b.def("defglobal", vec![Name], false, true, Some(Variable));
    b.def("defdynamic", vec![Name], false, true, Some(Variable));
    b.reg
}

/// AutoLISP's core definition forms.
pub fn autolisp_builtins() -> Registry {
    use Role::{Arglist, Name};
    let mut b = Builtins::new();
    b.def(
        "defun",
        vec![Name, Arglist],
        false,
        true,
        Some(Category::Function),
    );
    b.reg
}
