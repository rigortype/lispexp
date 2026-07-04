//! Reader tests for the Clojure dialect.

use lispexp::{parse, Datum, DatumKind, Delim, Options, Prefix};

fn clj(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::clojure());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn vectors_maps_and_sets() {
    let data = clj("[1 2] {:a 1} #{1 2}");
    let delims: Vec<Delim> = data
        .iter()
        .map(|d| match &d.kind {
            DatumKind::List { delim, .. } => *delim,
            other => panic!("expected list, got {:?}", other),
        })
        .collect();
    assert_eq!(delims, vec![Delim::Square, Delim::Curly, Delim::Set]);
}

#[test]
fn commas_are_whitespace() {
    let data = clj("[1, 2, 3]");
    let DatumKind::List { items, delim, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*delim, Delim::Square);
    assert_eq!(items.len(), 3);
}

#[test]
fn keywords() {
    let data = clj(":foo :ns/bar ::auto");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Keyword(":foo"),
            &DatumKind::Keyword(":ns/bar"),
            &DatumKind::Keyword("::auto"),
        ]
    );
}

#[test]
fn backslash_char_literals() {
    let data = clj(r"\a \newline \space");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Char(r"\a"),
            &DatumKind::Char(r"\newline"),
            &DatumKind::Char(r"\space"),
        ]
    );
}

#[test]
fn char_paren_does_not_open_a_list() {
    // `\(` is a character, not an open delimiter.
    let data = clj(r"(f \()");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].kind, DatumKind::Char(r"\("));
}

#[test]
fn anonymous_function_reader_macro() {
    let data = clj("#(+ % 1)");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!("expected #() as HashFn prefix")
    };
    assert_eq!(*prefix, Prefix::HashFn);
    let DatumKind::List { items, delim, .. } = &inner.kind else {
        panic!()
    };
    assert_eq!(*delim, Delim::Round);
    assert_eq!(items[0].kind, DatumKind::Symbol("+"));
}

#[test]
fn deref_and_var_quote_and_discard() {
    let data = clj("@atom #'foo #_ignored kept");
    // #_ discards the next form, so we expect: @atom, #'foo, kept.
    assert_eq!(data.len(), 3);
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Deref,
            ..
        }
    ));
    assert!(matches!(
        data[1].kind,
        DatumKind::Prefixed {
            prefix: Prefix::VarQuote,
            ..
        }
    ));
    assert_eq!(data[2].kind, DatumKind::Symbol("kept"));
}

#[test]
fn syntax_quote_unquote_splicing() {
    let data = clj("`(a ~b ~@c)");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*prefix, Prefix::Quasiquote);
    let DatumKind::List { items, .. } = &inner.kind else {
        panic!()
    };
    assert!(matches!(
        items[1].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Unquote,
            ..
        }
    ));
    assert!(matches!(
        items[2].kind,
        DatumKind::Prefixed {
            prefix: Prefix::UnquoteSplicing,
            ..
        }
    ));
}

#[test]
fn tagged_literals() {
    let data = clj(r#"#inst "2026-01-01" #uuid "x""#);
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected tagged literal")
    };
    assert_eq!(*tag, "inst");
    assert!(matches!(inner.as_ref().unwrap().kind, DatumKind::Str(_)));
    let DatumKind::HashLiteral { tag, .. } = &data[1].kind else {
        panic!()
    };
    assert_eq!(*tag, "uuid");
}

#[test]
fn regex_literal_is_a_leaf() {
    let data = clj(r#"#"\d+""#);
    assert_eq!(data.len(), 1);
    assert!(matches!(data[0].kind, DatumKind::Str(_)));
}

#[test]
fn reader_conditional() {
    let data = clj("#?(:clj 1 :cljs 2)");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*prefix, Prefix::ReaderConditional { splicing: false });
    assert!(matches!(inner.kind, DatumKind::List { .. }));
}

#[test]
fn metadata_wraps_target() {
    // Structure stays correct: one form, the vector, wrapped as Meta.
    let data = clj("^:dynamic [1 2]");
    assert_eq!(data.len(), 1);
    let DatumKind::Prefixed {
        prefix, inner, arg, ..
    } = &data[0].kind
    else {
        panic!("expected metadata-wrapped form")
    };
    assert_eq!(*prefix, Prefix::Meta);
    // The metadata form is retained as `arg` (T2), not dropped.
    let meta = arg.as_ref().expect("metadata retained in arg");
    assert_eq!(meta.kind, DatumKind::Keyword(":dynamic"));
    assert!(matches!(
        inner.kind,
        DatumKind::List {
            delim: Delim::Square,
            ..
        }
    ));
}

#[test]
fn ratios_and_symbols_and_nil() {
    let data = clj("1/2 foo/bar nil true false /");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Number("1/2"),
            &DatumKind::Symbol("foo/bar"),
            &DatumKind::Symbol("nil"),
            &DatumKind::Symbol("true"),
            &DatumKind::Symbol("false"),
            &DatumKind::Symbol("/"),
        ]
    );
}

#[test]
fn symbolic_values() {
    // ##Inf / ##-Inf / ##NaN are self-contained literals, not tagged forms.
    let data = clj("##Inf ##-Inf ##NaN");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Number("##Inf"),
            &DatumKind::Number("##-Inf"),
            &DatumKind::Number("##NaN"),
        ]
    );
}

#[test]
fn interop_dot_is_not_a_dotted_pair() {
    // Clojure has no dotted-pair syntax; `(. obj method)` is a 3-element list.
    let data = clj("(. obj method)");
    let DatumKind::List { items, tail, .. } = &data[0].kind else {
        panic!()
    };
    assert!(
        tail.is_none(),
        "`.` must not be read as a dotted tail in Clojure"
    );
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].kind, DatumKind::Symbol("."));
}

#[test]
fn quote_longhand_not_folded() {
    // Clojure must NOT fold `(quote x)` — its longhand spellings differ and `'`
    // is genuine reader syntax (T3). It stays a plain two-element list.
    let data = clj("(quote x)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected an unfolded list, got {:?}", data[0].kind)
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DatumKind::Symbol("quote"));
    assert_eq!(items[1].kind, DatumKind::Symbol("x"));
}

#[test]
fn discard_is_dropped_by_default() {
    // `#_` drops the next datum: the vector reads as `[a c]`.
    let data = clj("[a #_b c]");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    let names: Vec<_> = items
        .iter()
        .map(|d| match &d.kind {
            DatumKind::Symbol(s) => *s,
            _ => "?",
        })
        .collect();
    assert_eq!(names, ["a", "c"]);
}

#[test]
fn keep_discarded_retains_the_form() {
    // With `keep_discarded`, `#_b` stays as a `Prefixed { Discard, b }`, so a
    // round-trip consumer can still see its span and shape. `Options` is
    // `#[non_exhaustive]`, so a caller mutates the field rather than struct-updating.
    let mut opts = Options::clojure();
    opts.keep_discarded = true;
    let parsed = parse("[a #_(b c) d]", &opts);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let DatumKind::List { items, .. } = &parsed.data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].kind, DatumKind::Symbol("a"));
    assert_eq!(items[2].kind, DatumKind::Symbol("d"));
    let DatumKind::Prefixed { prefix, inner, .. } = &items[1].kind else {
        panic!("expected a kept discard, got {:?}", items[1].kind)
    };
    assert_eq!(*prefix, Prefix::Discard);
    // The discarded form's inner list is preserved with its real span.
    assert!(matches!(inner.kind, DatumKind::List { .. }));
    assert_eq!(
        &"[a #_(b c) d]"[items[1].span.start as usize..items[1].span.end as usize],
        "#_(b c)"
    );
}
