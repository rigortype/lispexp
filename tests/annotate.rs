//! Tests for the definition-form annotator (ADR-0019).

use lispexp::annotate::{
    annotate_form, annotate_tree, bundled_registry, harvest_source, Confidence, Docstring,
    Registry, Role,
};
use lispexp::{parse, DatumKind, Dialect, Options};

fn el_data(src: &str) -> Vec<lispexp::Datum<'_>> {
    parse(src, &Options::emacs_lisp()).data
}

fn el_reg() -> Registry {
    bundled_registry(Dialect::EmacsLisp)
}

#[test]
fn annotates_defun_with_docstring() {
    let reg = el_reg();
    let data = el_data(r#"(defun greet (name) "Say hi." (message "hi %s" name))"#);
    let a = annotate_form(&data[0], &reg).expect("defun should annotate");
    assert_eq!(a.head, "defun");
    assert_eq!(
        a.first(Role::Name).unwrap().kind,
        DatumKind::Symbol("greet")
    );
    assert!(matches!(
        a.first(Role::Arglist).unwrap().kind,
        DatumKind::List { .. }
    ));
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"Say hi.\"")
    );
    // The (message ...) call is the body.
    assert!(a.parts.iter().any(|p| p.role == Role::Body));
    // The Annotated carries the whole form, so its span is the full extent.
    assert_eq!(a.form.span, data[0].span);
}

#[test]
fn defun_without_docstring_has_no_docstring_part() {
    let reg = el_reg();
    let data = el_data("(defun add (a b) (+ a b))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Docstring).is_none());
    assert_eq!(a.first(Role::Name).unwrap().kind, DatumKind::Symbol("add"));
}

#[test]
fn elisp_lone_string_is_a_docstring() {
    // `(defun f () "hi")` — in elisp the lone body string IS the docstring
    // (`documentation` returns it), unlike CL where it is a return value.
    let reg = el_reg();
    let data = el_data(r#"(defun f () "hi")"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"hi\"")
    );
}

#[test]
fn cl_lone_string_is_a_value_not_docstring() {
    // Same shape under Common Lisp: the lone string is the return value
    // (CLHS 3.4.11).
    let reg = bundled_registry(Dialect::CommonLisp);
    let data = parse(r#"(defun f () "hi")"#, &Options::common_lisp()).data;
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Docstring).is_none());
    assert!(a.parts.iter().any(|p| p.role == Role::Body));
}

#[test]
fn defvar_value_is_not_docstring() {
    // `(defvar v "green" "doc")` — the value string must not be tagged as the
    // docstring; the trailing string is the doc.
    let reg = el_reg();
    let data = el_data(r#"(defvar v "green" "Cursor color.")"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"Cursor color.\"")
    );
    // And a doc-less defvar keeps its string value un-doc-tagged.
    let data = el_data(r#"(defvar w "just-a-value")"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Docstring).is_none());
}

#[test]
fn defcustom_standard_value_then_docstring() {
    let reg = el_reg();
    let data = el_data(r#"(defcustom my-opt " Lit" "The lighter." :type 'string)"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"The lighter.\"")
    );
}

#[test]
fn ert_deftest_has_mandatory_arglist() {
    let reg = el_reg();
    let data = el_data(r#"(ert-deftest my-test () "Doc." (should t))"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Arglist).is_some());
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"Doc.\"")
    );
}

#[test]
fn declare_and_interactive_are_tagged() {
    let reg = el_reg();
    let data = el_data("(defun cmd (x) \"doc\" (declare (indent 1)) (interactive \"p\") (list x))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.parts.iter().any(|p| p.role == Role::Declare));
    assert!(a.parts.iter().any(|p| p.role == Role::Interactive));
}

#[test]
fn cl_defun_is_a_builtin() {
    let reg = el_reg();
    let data = el_data("(cl-defun f (&key a b) (+ a b))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.head, "cl-defun");
    assert_eq!(a.confidence, Confidence::Builtin);
}

#[test]
fn non_definition_form_is_not_annotated() {
    let reg = el_reg();
    let data = el_data("(message \"hi\")");
    assert!(annotate_form(&data[0], &reg).is_none());
}

#[test]
fn annotate_tree_finds_nested_definitions() {
    let reg = el_reg();
    let data = el_data("(progn (defun a () 1) (defvar b 2 \"doc\"))");
    let all = annotate_tree(&data, &reg);
    let heads: Vec<&str> = all.iter().map(|a| a.head).collect();
    assert!(heads.contains(&"defun"));
    assert!(heads.contains(&"defvar"));
}

#[test]
fn harvest_infers_spec_from_param_names() {
    // A third-party def-macro with no &define spec — roles come from its own
    // parameter names.
    let mut reg = Registry::new();
    let added = harvest_source(
        "(cl-defmacro dirvish-define-preview (name &optional arglist docstring &rest body)\n\
           \"Define a preview dispatcher.\"\n\
           (list name arglist docstring body))",
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("dirvish-define-preview").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name, Role::Arglist]);
    assert_eq!(spec.docstring, Docstring::LeadingOrLone);
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Inferred);
}

#[test]
fn harvest_rest_args_param_is_body_not_arglist() {
    // `(defmacro m (name &rest args) …)` — the &rest param stands for the
    // remainder; it must NOT become a fixed Arglist slot that eats the first
    // body form.
    let mut reg = Registry::new();
    harvest_source("(defmacro with-thing (name &rest args) nil)", &mut reg);
    let spec = reg.get("with-thing").unwrap();
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);

    let data = el_data("(with-thing foo (first) (second))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.parts.iter().filter(|p| p.role == Role::Body).count(), 2);
    assert!(a.first(Role::Arglist).is_none());
}

#[test]
fn harvest_then_annotate_end_to_end() {
    let mut reg = el_reg();
    harvest_source(
        "(defmacro my-defcommand (name &rest body) (declare (doc-string 2) (indent 1)) body)",
        &mut reg,
    );
    // The declare (doc-string ...) makes it a Declared spec.
    assert_eq!(
        reg.get("my-defcommand").unwrap().confidence,
        Confidence::Declared
    );
    let data = el_data(r#"(my-defcommand foo "the docs" (do-thing) (do-other))"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.head, "my-defcommand");
    assert_eq!(a.first(Role::Name).unwrap().kind, DatumKind::Symbol("foo"));
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"the docs\"")
    );
    assert_eq!(a.parts.iter().filter(|p| p.role == Role::Body).count(), 2);
}

#[test]
fn harvest_ignores_non_defmacro() {
    let mut reg = Registry::new();
    let added = harvest_source("(defun foo () 1)\n(setq x 2)", &mut reg);
    assert_eq!(added, 0);
    assert!(reg.is_empty());
}

#[test]
fn anonymous_fennel_fn_is_not_annotated() {
    // `(fn [x] x)` — no name; the arglist must not be mis-tagged as the Name.
    let reg = bundled_registry(Dialect::Fennel);
    let data = parse("(fn [x] x)", &Options::fennel()).data;
    assert!(annotate_form(&data[0], &reg).is_none());
    // The named form still annotates.
    let data = parse("(fn add [x y] (+ x y))", &Options::fennel()).data;
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.first(Role::Name).unwrap().kind, DatumKind::Symbol("add"));
}

#[test]
fn setf_function_name_is_name_shaped() {
    // CL `(defun (setf foo) (v) …)` — a round-list name is accepted.
    let reg = bundled_registry(Dialect::CommonLisp);
    let data = parse("(defun (setf foo) (v x) v)", &Options::common_lisp()).data;
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(matches!(
        a.first(Role::Name).unwrap().kind,
        DatumKind::List { .. }
    ));
}

#[test]
fn registry_composes() {
    use lispexp::annotate::FormSpec;
    let mut reg = bundled_registry(Dialect::EmacsLisp);
    let n = reg.len();
    // merge: later layer wins on collision.
    let mut overlay = Registry::new();
    overlay.insert(FormSpec::define(
        "defun",
        vec![Role::Name],
        Docstring::None,
        true,
    ));
    reg.merge(overlay);
    assert_eq!(reg.len(), n);
    assert_eq!(reg.get("defun").unwrap().confidence, Confidence::Consumer);
    // iter + collect round-trips.
    let copied: Registry = reg.iter().cloned().collect();
    assert_eq!(copied.len(), reg.len());
    // remove.
    reg.remove("defun");
    assert!(reg.get("defun").is_none());
}
