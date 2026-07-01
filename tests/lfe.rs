//! Reader tests for the LFE (Lisp Flavoured Erlang) dialect.

use sexpp::{parse, Datum, DatumKind, Delim, Options, Prefix};

fn lfe(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::lfe());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn hash_paren_tuple() {
    let data = lfe("#(1 2)");
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected tuple, got {:?}", data[0].kind)
    };
    assert_eq!(*tag, "");
    assert!(matches!(
        inner.as_ref().unwrap().kind,
        DatumKind::List { .. }
    ));
}

#[test]
fn binary_string() {
    let data = lfe(r#"#"abc""#);
    assert!(matches!(data[0].kind, DatumKind::Str(_)));
}

#[test]
fn function_quote_with_arity() {
    let data = lfe("#'foo/2");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*prefix, Prefix::FunctionQuote);
    assert_eq!(inner.kind, DatumKind::Symbol("foo/2"));
}

#[test]
fn square_is_list_shape() {
    let data = lfe("[a b]");
    assert!(matches!(
        data[0].kind,
        DatumKind::List {
            delim: Delim::Square,
            ..
        }
    ));
}

#[test]
fn dotted_pair() {
    let data = lfe("(a . b)");
    let DatumKind::List { items, tail, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(tail.as_ref().unwrap().kind, DatumKind::Symbol("b"));
}

#[test]
fn non_nestable_block_comment_is_skipped() {
    let data = lfe("#| a |# (foo)");
    assert_eq!(data.len(), 1);
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items[0].kind, DatumKind::Symbol("foo"));
}

#[test]
fn colon_prefixed_is_a_symbol() {
    // Colon is an ordinary symbol character in LFE; not a keyword.
    let data = lfe(":foo");
    assert_eq!(data[0].kind, DatumKind::Symbol(":foo"));
}
