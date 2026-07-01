//! Tests for the definition-form annotator (ADR-0019).

use lispexp::annotate::{
    annotate_form, annotate_tree, emacs_lisp_builtins, harvest_source, Confidence, Registry, Role,
};
use lispexp::{parse, DatumKind, Options};

fn el_data(src: &str) -> Vec<lispexp::Datum<'_>> {
    parse(src, &Options::emacs_lisp()).data
}

#[test]
fn annotates_defun_with_docstring() {
    let reg = emacs_lisp_builtins();
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
}

#[test]
fn defun_without_docstring_has_no_docstring_part() {
    let reg = emacs_lisp_builtins();
    let data = el_data("(defun add (a b) (+ a b))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Docstring).is_none());
    assert_eq!(a.first(Role::Name).unwrap().kind, DatumKind::Symbol("add"));
}

#[test]
fn lone_trailing_string_is_body_not_docstring() {
    // `(defun f () "hi")` — the string is the return value, not a docstring.
    let reg = emacs_lisp_builtins();
    let data = el_data(r#"(defun f () "hi")"#);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Docstring).is_none());
    assert!(a.parts.iter().any(|p| p.role == Role::Body));
}

#[test]
fn declare_and_interactive_are_tagged() {
    let reg = emacs_lisp_builtins();
    let data = el_data("(defun cmd (x) \"doc\" (declare (indent 1)) (interactive \"p\") (list x))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.parts.iter().any(|p| p.role == Role::Declare));
    assert!(a.parts.iter().any(|p| p.role == Role::Interactive));
}

#[test]
fn cl_defun_is_a_builtin() {
    let reg = emacs_lisp_builtins();
    let data = el_data("(cl-defun f (&key a b) (+ a b))");
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.head, "cl-defun");
    assert_eq!(a.confidence, Confidence::Builtin);
}

#[test]
fn non_definition_form_is_not_annotated() {
    let reg = emacs_lisp_builtins();
    let data = el_data("(message \"hi\")");
    assert!(annotate_form(&data[0], &reg).is_none());
}

#[test]
fn annotate_tree_finds_nested_definitions() {
    let reg = emacs_lisp_builtins();
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
    assert!(spec.docstring);
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Inferred);
}

#[test]
fn harvest_then_annotate_end_to_end() {
    let mut reg = emacs_lisp_builtins();
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
