//! Reader tests for the Fennel dialect.

use sexpp::{parse, Datum, DatumKind, Delim, Options, Prefix};

fn fnl(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::fennel());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn square_is_list_shape() {
    let data = fnl("[1 2]");
    assert!(matches!(
        data[0].kind,
        DatumKind::List {
            delim: Delim::Square,
            ..
        }
    ));
}

#[test]
fn curly_is_table_shape() {
    let data = fnl("{:a 1}");
    let DatumKind::List { delim, items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*delim, Delim::Curly);
    // keyword_colon is false, so `:a` is a plain symbol leaf.
    assert_eq!(items[0].kind, DatumKind::Symbol(":a"));
}

#[test]
fn hash_paren_is_hashfn() {
    let data = fnl("#(+ $1 1)");
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::HashFn,
            ..
        }
    ));
}

#[test]
fn fn_form_with_square_params() {
    let data = fnl("(fn [x] x)");
    assert_eq!(data.len(), 1);
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items[0].kind, DatumKind::Symbol("fn"));
    assert!(matches!(
        items[1].kind,
        DatumKind::List {
            delim: Delim::Square,
            ..
        }
    ));
}

#[test]
fn true_false_nil_are_symbols() {
    let data = fnl("true false nil");
    assert_eq!(data[0].kind, DatumKind::Symbol("true"));
    assert_eq!(data[1].kind, DatumKind::Symbol("false"));
    assert_eq!(data[2].kind, DatumKind::Symbol("nil"));
}

#[test]
fn colon_prefixed_is_a_symbol_not_keyword() {
    let data = fnl(":foo");
    assert_eq!(data[0].kind, DatumKind::Symbol(":foo"));
}

#[test]
fn line_comment_is_skipped() {
    let data = fnl("; a comment\n(print :ok)");
    assert_eq!(data.len(), 1);
    assert!(matches!(data[0].kind, DatumKind::List { .. }));
}
