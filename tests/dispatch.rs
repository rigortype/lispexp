//! Dispatch/method form annotation (ADR-0021).

use lispexp::annotate::{annotate_form, bundled_registry, Role};
use lispexp::{parse, DatumKind, Dialect, Options};

fn annotate<'a>(
    src: &'a str,
    dialect: Dialect,
) -> (Vec<lispexp::Datum<'a>>, lispexp::annotate::Registry) {
    let data = parse(src, &Options::for_dialect(dialect)).data;
    (data, bundled_registry(dialect))
}

#[test]
fn cl_defmethod_with_qualifier_and_specializers() {
    let (data, reg) = annotate(
        "(cl-defmethod foo :around ((x integer) (y string)) (bar))",
        Dialect::EmacsLisp,
    );
    let a = annotate_form(&data[0], &reg).unwrap();

    assert_eq!(a.first(Role::Name).unwrap().kind, DatumKind::Symbol("foo"));

    // The qualifier `:around` is read as a token between name and arglist.
    let quals: Vec<_> = a.all(Role::Qualifier).collect();
    assert_eq!(quals.len(), 1);
    assert_eq!(quals[0].kind, DatumKind::Keyword(":around"));

    // The specialized arglist splits into (variable, specializer) pairs.
    let params = a.specialized_params();
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].variable.kind, DatumKind::Symbol("x"));
    assert_eq!(
        params[0].specializer.unwrap().kind,
        DatumKind::Symbol("integer")
    );
    assert_eq!(params[1].variable.kind, DatumKind::Symbol("y"));
    assert_eq!(
        params[1].specializer.unwrap().kind,
        DatumKind::Symbol("string")
    );
}

#[test]
fn cl_defmethod_zero_qualifiers() {
    let (data, reg) = annotate(
        "(cl-defmethod area ((s square)) (* (side s) (side s)))",
        Dialect::EmacsLisp,
    );
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.all(Role::Qualifier).count(), 0);
    let params = a.specialized_params();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].variable.kind, DatumKind::Symbol("s"));
    assert_eq!(
        params[0].specializer.unwrap().kind,
        DatumKind::Symbol("square")
    );
}

#[test]
fn cl_defmethod_multiple_qualifiers() {
    // ANSI CL allows several qualifiers; all are captured.
    let (data, reg) = annotate(
        "(defmethod foo :before :extra ((x t)) nil)",
        Dialect::CommonLisp,
    );
    let a = annotate_form(&data[0], &reg).unwrap();
    let quals: Vec<_> = a.all(Role::Qualifier).map(|d| &d.kind).collect();
    assert_eq!(quals.len(), 2);
}

#[test]
fn eql_specializer_is_verbatim_list() {
    let (data, reg) = annotate("(cl-defmethod g ((x (eql 0))) x)", Dialect::EmacsLisp);
    let a = annotate_form(&data[0], &reg).unwrap();
    let params = a.specialized_params();
    assert_eq!(params[0].variable.kind, DatumKind::Symbol("x"));
    // The specializer is the whole `(eql 0)` list, unresolved.
    assert!(matches!(
        params[0].specializer.unwrap().kind,
        DatumKind::List { .. }
    ));
}

#[test]
fn unspecialized_param_has_no_specializer() {
    let (data, reg) = annotate("(cl-defmethod h ((x integer) y) x)", Dialect::EmacsLisp);
    let a = annotate_form(&data[0], &reg).unwrap();
    let params = a.specialized_params();
    assert_eq!(params.len(), 2);
    assert!(params[1].specializer.is_none());
    assert_eq!(params[1].variable.kind, DatumKind::Symbol("y"));
}

#[test]
fn cl_defmethod_docstring_is_tagged() {
    let (data, reg) = annotate(
        r#"(defmethod area ((s square)) "Compute the area." (* s s))"#,
        Dialect::CommonLisp,
    );
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(
        a.first(Role::Docstring).unwrap().kind,
        DatumKind::Str("\"Compute the area.\"")
    );
}

#[test]
fn clojure_multi_arity_defmethod_clause_is_not_arglist() {
    // `(defmethod f :x ([a] …) ([a b] …))` — the round arity clauses must not
    // be tagged as the Arglist (Clojure arglists are square vectors).
    let (data, reg) = annotate("(defmethod f :x ([a] a) ([a b] b))", Dialect::Clojure);
    let a = annotate_form(&data[0], &reg).unwrap();
    assert!(a.first(Role::Arglist).is_none());
    assert_eq!(a.parts.iter().filter(|p| p.role == Role::Body).count(), 2);
}

#[test]
fn clojure_defmethod_uses_dispatch_value() {
    let (data, reg) = annotate(
        "(defmethod area :circle [shape] (* 3 (:r shape)))",
        Dialect::Clojure,
    );
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.first(Role::Name).unwrap().kind, DatumKind::Symbol("area"));
    // The dispatch value is a single datum, not a qualifier.
    assert_eq!(
        a.first(Role::DispatchValue).unwrap().kind,
        DatumKind::Keyword(":circle")
    );
    assert_eq!(a.all(Role::Qualifier).count(), 0);
    // The plain arglist follows.
    assert!(matches!(
        a.first(Role::Arglist).unwrap().kind,
        DatumKind::List { .. }
    ));
}
