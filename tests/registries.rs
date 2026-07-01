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
    assert!(reg.get("define-syntax").is_some()); // inherited from Scheme
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
