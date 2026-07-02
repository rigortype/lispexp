//! Tests for the definition-form annotator (ADR-0019).

use lispexp::annotate::{
    annotate_form, annotate_tree, bundled_registry, harvest_source, harvest_source_for, Confidence,
    Docstring, Registry, Role,
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
fn harvest_common_lisp_defmacro_body_marker() {
    // CL `defmacro` with a `&body` lambda-list marker (ADR-0032): the arglist
    // names `name`/`args`, `&body forms` opens the body.
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(defmacro define-widget (name args &body forms) `(progn ,@forms))",
        Dialect::CommonLisp,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("define-widget").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name, Role::Arglist]);
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Inferred);
}

#[test]
fn harvest_clojure_defmacro_vector_arglist() {
    // Clojure def-macro: a `[name & body]` *vector* arglist with the `&` rest
    // marker (ADR-0032).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(defmacro defwidget [name & body] `(def ~name ~@body))",
        Dialect::Clojure,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("defwidget").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_clojure_docstring_before_arglist() {
    // Clojure puts the docstring *before* the arglist; the harvester skips it
    // to find the params (ADR-0032).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(defmacro defwidget \"A widget.\" [name & body] `(def ~name ~@body))",
        Dialect::Clojure,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("defwidget").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_clojure_arglists_metadata_is_authoritative() {
    // `:arglists` overrides the (uninformative) param vector and is Declared
    // provenance — the analog of elisp `declare` (ADR-0032).
    let mut reg = Registry::new();
    harvest_source_for(
        "(defmacro deftask {:arglists '([name docstring & body])} [& args] `(do ~@args))",
        Dialect::Clojure,
        &mut reg,
    );
    let spec = reg.get("deftask").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert_eq!(spec.docstring, Docstring::Leading);
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Declared);
}

#[test]
fn harvest_clojure_reader_metadata_on_name() {
    // `^{:arglists …}` reader metadata rides on the name symbol.
    let mut reg = Registry::new();
    harvest_source_for(
        "(defmacro ^{:arglists '([name & body])} defthing [& args] nil)",
        Dialect::Clojure,
        &mut reg,
    );
    let spec = reg.get("defthing").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Declared);
}

#[test]
fn harvest_clojure_style_indent_sets_body_boundary() {
    // `:style/indent 2` with an opaque `[& args]` vector: two leading
    // distinguished args, then a body (ADR-0032, the analog of elisp indent).
    let mut reg = Registry::new();
    harvest_source_for(
        "(defmacro deffoo {:style/indent 2} [& args] `(do ~@args))",
        Dialect::Clojure,
        &mut reg,
    );
    let spec = reg.get("deffoo").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Other, Role::Other]);
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Declared);
}

#[test]
fn harvest_clojure_arglists_wins_over_style_indent() {
    // When both are present, `:arglists` (which names roles) is authoritative;
    // `:style/indent` does not pad past it.
    let mut reg = Registry::new();
    harvest_source_for(
        "(defmacro defbar {:style/indent 3 :arglists '([name & body])} [& args] nil)",
        Dialect::Clojure,
        &mut reg,
    );
    let spec = reg.get("defbar").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_clojure_style_indent_nested_list_uses_head() {
    // The nested `[n …]` form: the head element `2` is the form-level indent
    // (ADR-0032). Written as a Clojure vector.
    let mut reg = Registry::new();
    harvest_source_for(
        "(defmacro defnested {:style/indent [2 [1]]} [& args] nil)",
        Dialect::Clojure,
        &mut reg,
    );
    let spec = reg.get("defnested").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Other, Role::Other]);
    assert!(spec.body);
}

#[test]
fn harvest_clojure_style_indent_defn_keyword_is_body() {
    let mut reg = Registry::new();
    harvest_source_for(
        "(defmacro defbaz {:style/indent :defn} [& args] nil)",
        Dialect::Clojure,
        &mut reg,
    );
    let spec = reg.get("defbaz").expect("harvested");
    assert!(spec.leading.is_empty());
    assert!(spec.body);
    assert_eq!(spec.confidence, Confidence::Declared);
}

#[test]
fn harvest_hy_hash_star_rest_is_body() {
    // Hy writes rest params as `#* body`, which read as a `#`-tagged datum, not
    // a `&`-marked symbol.
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(defmacro defthing [name #* body] name)",
        Dialect::Hy,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("defthing").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_fennel_macro_head() {
    // Fennel defines macros with `macro`, not `defmacro` (ADR-0032).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(macro defthing [name body] `(local ,name ,body))",
        Dialect::Fennel,
        &mut reg,
    );
    assert_eq!(added, 1);
    assert!(reg.get("defthing").is_some());
}

#[test]
fn harvest_scheme_syntax_rules_pattern() {
    // The syntax-rules pattern `(_ name (arg ...) body ...)` names and nests the
    // roles: Name, Arglist, then a body tail (ADR-0031).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(define-syntax define-test\n\
           (syntax-rules ()\n\
             ((_ name (arg ...) body ...) (define (name arg ...) body ...))))",
        Dialect::Scheme,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("define-test").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name, Role::Arglist]);
    assert!(spec.body);
    assert_eq!(spec.docstring, Docstring::None); // Scheme has no docstrings
    assert_eq!(spec.confidence, Confidence::Inferred);
}

#[test]
fn harvest_scheme_syntax_rules_trailing_ellipsis_is_body() {
    // `(_ name body ...)` → Name, then the repeated tail is the body.
    let mut reg = Registry::new();
    harvest_source_for(
        "(define-syntax defthing (syntax-rules () ((_ name body ...) (begin body ...))))",
        Dialect::Scheme,
        &mut reg,
    );
    let spec = reg.get("defthing").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_scheme_picks_richest_rule() {
    // A multi-rule macro: the nullary `(_)` rule yields nothing; the richest
    // rule wins.
    let mut reg = Registry::new();
    harvest_source_for(
        "(define-syntax my-def\n\
           (syntax-rules ()\n\
             ((_ name) (define name #f))\n\
             ((_ name body ...) (define name (begin body ...)))))",
        Dialect::Scheme,
        &mut reg,
    );
    let spec = reg.get("my-def").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_racket_define_syntax_rule() {
    // `(define-syntax-rule (name pat…) template)` — the name is the head of the
    // pattern; the rest are the args (ADR-0031).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(define-syntax-rule (define-thing name body) (define name body))",
        Dialect::Racket,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("define-thing").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_racket_syntax_parse_strips_syntax_class() {
    // syntax-parse pattern with a `name:id` syntax class — stripped to `name`.
    let mut reg = Registry::new();
    harvest_source_for(
        "(define-syntax (define-check stx)\n\
           (syntax-parse stx\n\
             [(_ name:id body ...) #'(define (name) body ...)]))",
        Dialect::Racket,
        &mut reg,
    );
    let spec = reg.get("define-check").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_racket_syntax_case_in_lambda() {
    // A `syntax-case` transformer wrapped in a `lambda` is reached by the
    // recursive search.
    let mut reg = Registry::new();
    harvest_source_for(
        "(define-syntax define-thing\n\
           (lambda (stx)\n\
             (syntax-case stx ()\n\
               ((_ name val) (syntax (define name val))))))",
        Dialect::Racket,
        &mut reg,
    );
    let spec = reg.get("define-thing").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name, Role::Other]);
    assert!(!spec.body);
}

#[test]
fn harvest_guile_define_macro_dotted_tail_is_body() {
    // Guile/Gauche legacy `(define-macro (name arg . body) …)`: the signature is
    // an arglist and the dotted tail is the body (ADR-0031).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(define-macro (define-thing name . body) (cons name body))",
        Dialect::Guile,
        &mut reg,
    );
    assert_eq!(added, 1);
    let spec = reg.get("define-thing").expect("harvested");
    assert_eq!(spec.leading, vec![Role::Name]);
    assert!(spec.body);
}

#[test]
fn harvest_guile_define_macro_procedural_is_skipped() {
    // `(define-macro name (lambda …))` has a symbol, not a signature list, so
    // there is no arglist to harvest.
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(define-macro my-macro (lambda (form) form))",
        Dialect::Guile,
        &mut reg,
    );
    assert_eq!(added, 0);
}

#[test]
fn harvest_scheme_skips_procedural_transformer() {
    // `er-macro-transformer` carries no pattern — nothing to harvest (ADR-0031).
    let mut reg = Registry::new();
    let added = harvest_source_for(
        "(define-syntax my-macro (er-macro-transformer (lambda (form rename compare) form)))",
        Dialect::Scheme,
        &mut reg,
    );
    assert_eq!(added, 0);
}

#[test]
fn harvest_scheme_end_to_end_annotates_use() {
    // Harvest the macro, then annotate a *use* of it.
    let mut reg = bundled_registry(Dialect::Scheme);
    harvest_source_for(
        "(define-syntax define-test\n\
           (syntax-rules () ((_ name body ...) (define (name) body ...))))",
        Dialect::Scheme,
        &mut reg,
    );
    let data = parse("(define-test my-check (assert #t))", &Options::scheme()).data;
    let a = annotate_form(&data[0], &reg).expect("annotated");
    assert_eq!(
        a.first(Role::Name).unwrap().kind,
        DatumKind::Symbol("my-check")
    );
    assert!(a.all(Role::Body).count() >= 1);
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

// --- Descent follows code-vs-data classification (ADR-0026) ----------------

fn cl_reg() -> Registry {
    bundled_registry(Dialect::CommonLisp)
}

fn cl_heads(src: &str) -> Vec<String> {
    let data = parse(src, &Options::common_lisp()).data;
    annotate_tree(&data, &cl_reg())
        .iter()
        .map(|a| a.head.to_string())
        .collect()
}

#[test]
fn annotates_defun_guarded_by_feature_conditional() {
    // `#+sbcl (defun …)` is code — the guarded form is evaluated when the
    // feature matches — so its definition must be reached (previously missed:
    // the old descent recursed only into lists, never into the `#+` wrapper).
    let data = parse("#+sbcl (defun only-sbcl () 1)", &Options::common_lisp()).data;
    let found = annotate_tree(&data, &cl_reg());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].head, "defun");
    assert_eq!(
        found[0].first(Role::Name).unwrap().kind,
        DatumKind::Symbol("only-sbcl")
    );
}

#[test]
fn does_not_annotate_quoted_definition() {
    // `'(defun …)` is inert data — a quoted list, never evaluated — so it must
    // NOT be annotated, even nested inside real code.
    assert!(
        cl_heads("(list '(defun fake () 1))").is_empty(),
        "quoted defun must not annotate"
    );
}

#[test]
fn does_not_annotate_quasiquote_template_definition() {
    // A `defun` in a quasiquote *template* (not unquoted) is data at that depth
    // — a macro building code, not a definition itself.
    assert!(
        cl_heads("`(progn (defun tmpl () 1))").is_empty(),
        "template defun must not annotate"
    );
}

#[test]
fn annotates_unquoted_definition_inside_quasiquote() {
    // ...but an *unquoted* form flips back to code, so it is reached.
    let data = parse("`(progn ,(defun spliced () 1))", &Options::common_lisp()).data;
    let found = annotate_tree(&data, &cl_reg());
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].first(Role::Name).unwrap().kind,
        DatumKind::Symbol("spliced")
    );
}

#[test]
fn still_annotates_nested_defs_outer_before_inner() {
    // Ordinary list descent is unchanged: nested defs found, outer before inner.
    assert_eq!(
        cl_heads("(progn (defun outer () (defun inner () 1)) (defmacro m () 2))"),
        vec!["defun", "defun", "defmacro"]
    );
}
