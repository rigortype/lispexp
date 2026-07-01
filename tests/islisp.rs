//! Reader tests for the ISLisp (ISO/IEC 13816) dialect.

use lispexp::{parse, Datum, DatumKind, Options, Prefix};

fn isl(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::islisp());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn keyword_colon() {
    let data = isl(":kw");
    assert_eq!(data[0].kind, DatumKind::Keyword(":kw"));
}

#[test]
fn function_quote() {
    let data = isl("#'car");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*prefix, Prefix::FunctionQuote);
    assert_eq!(inner.kind, DatumKind::Symbol("car"));
}

#[test]
fn hash_paren_vector() {
    let data = isl("#(1 2)");
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected vector, got {:?}", data[0].kind)
    };
    assert_eq!(*tag, "");
    assert!(matches!(
        inner.as_ref().unwrap().kind,
        DatumKind::List { .. }
    ));
}

#[test]
fn t_and_nil_are_symbols() {
    let data = isl("t nil");
    assert_eq!(data[0].kind, DatumKind::Symbol("t"));
    assert_eq!(data[1].kind, DatumKind::Symbol("nil"));
}

#[test]
fn dotted_pair() {
    let data = isl("(a . b)");
    let DatumKind::List { items, tail, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(tail.as_ref().unwrap().kind, DatumKind::Symbol("b"));
}

#[test]
fn char_literal() {
    let data = isl(r"#\a");
    assert_eq!(data[0].kind, DatumKind::Char(r"#\a"));
}

#[test]
fn piped_symbol_with_spaces() {
    let data = isl("|piped sym|");
    assert_eq!(data[0].kind, DatumKind::Symbol("|piped sym|"));
}

#[test]
fn square_brackets_are_ordinary_symbol_chars() {
    let data = isl("[a]");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].kind, DatumKind::Symbol("[a]"));
}
