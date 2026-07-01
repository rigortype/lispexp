//! Reader tests for the Guile dialect.

use lispexp::{parse, Datum, DatumKind, Options, Prefix};

fn guile(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::guile());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn hash_colon_keyword() {
    let data = guile("#:kw");
    assert_eq!(data[0].kind, DatumKind::Keyword("#:kw"));
}

#[test]
fn hash_apostrophe_is_var_quote() {
    let data = guile("#'x");
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::VarQuote,
            ..
        }
    ));
}

#[test]
fn booleans() {
    let data = guile("#t #f");
    assert_eq!(data[0].kind, DatumKind::Bool(true));
    assert_eq!(data[1].kind, DatumKind::Bool(false));
}

#[test]
fn char_literal() {
    let data = guile(r"#\a");
    assert_eq!(data[0].kind, DatumKind::Char(r"#\a"));
}

#[test]
fn nestable_block_comment_is_skipped() {
    let data = guile("#| a #| nested |# b |# (define x 1)");
    assert_eq!(data.len(), 1);
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items[0].kind, DatumKind::Symbol("define"));
    assert_eq!(items[1].kind, DatumKind::Symbol("x"));
    assert_eq!(items[2].kind, DatumKind::Number("1"));
}

#[test]
fn define_list() {
    let data = guile("(define (f x) (+ x 1))");
    assert_eq!(data.len(), 1);
    assert!(matches!(data[0].kind, DatumKind::List { .. }));
}
