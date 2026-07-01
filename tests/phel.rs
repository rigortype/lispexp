//! Reader tests for the Phel dialect (essentially Clojure's reader).

use sexpp::{parse, Datum, DatumKind, Delim, Options, Prefix};

fn phel(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::phel());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn square_is_list_shape() {
    let data = phel("[1 2]");
    assert!(matches!(
        data[0].kind,
        DatumKind::List {
            delim: Delim::Square,
            ..
        }
    ));
}

#[test]
fn curly_is_map_shape() {
    let data = phel("{:a 1}");
    let DatumKind::List { delim, items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*delim, Delim::Curly);
    assert_eq!(items[0].kind, DatumKind::Keyword(":a"));
}

#[test]
fn hash_curly_is_set_shape() {
    let data = phel("#{1 2}");
    assert!(matches!(
        data[0].kind,
        DatumKind::List {
            delim: Delim::Set,
            ..
        }
    ));
}

#[test]
fn keyword_colon() {
    let data = phel(":kw");
    assert_eq!(data[0].kind, DatumKind::Keyword(":kw"));
}

#[test]
fn hash_paren_is_hashfn() {
    let data = phel("#(+ % 1)");
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::HashFn,
            ..
        }
    ));
}

#[test]
fn commas_are_whitespace() {
    let data = phel("(1, 2)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DatumKind::Number("1"));
    assert_eq!(items[1].kind, DatumKind::Number("2"));
}
