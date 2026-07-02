//! Per-dialect bundled registries and the category hint (ADR-0020).

use lispexp::annotate::{annotate_form, bundled_registry, Category};
use lispexp::{parse, Dialect, Options};

fn annotate_head(src: &str, dialect: Dialect) -> (String, Option<Category>) {
    let opts = Options::for_dialect(dialect);
    let data = parse(src, &opts).data;
    let reg = bundled_registry(dialect);
    let a = annotate_form(&data[0], &reg).expect("form should annotate");
    (a.head.to_string(), a.category)
}

#[test]
fn clojure_defn_is_function() {
    let (head, cat) = annotate_head("(defn f [x] x)", Dialect::Clojure);
    assert_eq!(head, "defn");
    assert_eq!(cat, Some(Category::Function));
}

#[test]
fn clojure_def_is_ambiguous_no_category() {
    // `def` may bind a value or a function — kept as Kind only (ADR-0020).
    let (head, cat) = annotate_head("(def x 1)", Dialect::Clojure);
    assert_eq!(head, "def");
    assert_eq!(cat, None);
}

#[test]
fn common_lisp_defclass_is_class() {
    let (_, cat) = annotate_head("(defclass point () ())", Dialect::CommonLisp);
    assert_eq!(cat, Some(Category::Class));
}

#[test]
fn common_lisp_deftype_is_type() {
    let (head, cat) = annotate_head("(deftype id () t)", Dialect::CommonLisp);
    assert_eq!(head, "deftype");
    assert_eq!(cat, Some(Category::Type));
}

#[test]
fn scheme_define_syntax_is_macro() {
    let (_, cat) = annotate_head("(define-syntax swap! (syntax-rules () ))", Dialect::Scheme);
    assert_eq!(cat, Some(Category::Macro));
}

#[test]
fn scheme_define_is_ambiguous() {
    let (_, cat) = annotate_head("(define x 1)", Dialect::Scheme);
    assert_eq!(cat, None);
}

#[test]
fn phel_shares_clojure_core() {
    let (head, cat) = annotate_head("(defn f [x] x)", Dialect::Phel);
    assert_eq!(head, "defn");
    assert_eq!(cat, Some(Category::Function));
}

#[test]
fn racket_adds_struct_to_scheme_core() {
    let reg = bundled_registry(Dialect::Racket);
    assert!(reg.get("struct").is_some());
    assert!(reg.get("define-syntax-rule").is_some());
    assert!(reg.get("define-syntax").is_some()); // inherited from Scheme
}

#[test]
fn scheme_core_has_define_library_but_not_goops() {
    // Strict R7RS-small stays R7RS-faithful (ADR-0031): `define-library` yes,
    // GOOPS/Gauche forms no.
    let reg = bundled_registry(Dialect::Scheme);
    assert!(reg.get("define-library").is_some());
    assert!(reg.get("define-class").is_none());
    assert!(reg.get("define-syntax-rule").is_none());
}

#[test]
fn extended_scheme_family_adds_goops_and_gauche_forms() {
    // Guile/Gauche/Mosh/Gambit/superset layer implementation-common forms on
    // the R7RS core (ADR-0031).
    for d in [
        Dialect::Guile,
        Dialect::Gauche,
        Dialect::Mosh,
        Dialect::Gambit,
        Dialect::SchemeSuperset,
    ] {
        let reg = bundled_registry(d);
        assert!(
            reg.get("define-syntax").is_some(),
            "{d:?} keeps the R7RS core"
        );
        assert!(reg.get("define-class").is_some(), "{d:?} adds define-class");
        assert!(
            reg.get("define-inline").is_some(),
            "{d:?} adds define-inline"
        );
        // `define-method` is deliberately not bundled (non-uniform shape).
        assert!(
            reg.get("define-method").is_none(),
            "{d:?} omits define-method"
        );
    }
}

#[test]
fn gauche_define_class_is_class() {
    let (head, cat) = annotate_head("(define-class <point> () (x y))", Dialect::Gauche);
    assert_eq!(head, "define-class");
    assert_eq!(cat, Some(Category::Class));
}

#[test]
fn guile_define_star_is_ambiguous() {
    // `define*` is `define`-shaped — value vs. procedure — so no category.
    let (head, cat) = annotate_head("(define* (f x #:optional y) x)", Dialect::Guile);
    assert_eq!(head, "define*");
    assert_eq!(cat, None);
}

#[test]
fn edn_has_no_definitions() {
    assert!(bundled_registry(Dialect::Edn).is_empty());
}

#[test]
fn consumer_can_extend_bundled_core() {
    use lispexp::annotate::{Confidence, Docstring, FormSpec, Role};
    // The bundled core is composable: a consumer overrides/extends it.
    let mut reg = bundled_registry(Dialect::Clojure);
    reg.insert(
        FormSpec::define("defwidget", vec![Role::Name], Docstring::Leading, true)
            .with_category(Category::Macro),
    );
    let data = parse("(defwidget button [] :ok)", &Options::clojure()).data;
    let a = annotate_form(&data[0], &reg).unwrap();
    assert_eq!(a.head, "defwidget");
    assert_eq!(a.category, Some(Category::Macro));
    // Consumer-supplied specs carry their own provenance.
    assert_eq!(a.confidence, Confidence::Consumer);
}
