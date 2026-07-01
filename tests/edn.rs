//! Reader tests for the EDN dialect — a data-only preset on Clojure (ADR-0025).

use lispexp::{parse, Datum, DatumKind, Delim, Dialect, Options, Prefix};

fn edn(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::edn());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn deps_edn_map_parses() {
    let src = r#"{:deps {org/lib {:mvn/version "1.0"}}
                 :paths ["src" "resources"]}"#;
    let data = edn(src);
    assert_eq!(data.len(), 1);
    let DatumKind::List { delim, items, .. } = &data[0].kind else {
        panic!("expected a map");
    };
    assert_eq!(*delim, Delim::Curly);
    assert_eq!(items[0].kind, DatumKind::Keyword(":deps"));
}

#[test]
fn tagged_elements_stay_on() {
    let data = edn(r#"#inst "2020-01-01""#);
    assert!(matches!(
        &data[0].kind,
        DatumKind::HashLiteral { tag: "inst", .. }
    ));
}

#[test]
fn namespaced_map_stays_on() {
    let data = edn("#:ns{:a 1}");
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected a namespaced-map marker");
    };
    assert_eq!(*tag, ":ns");
    assert!(matches!(
        inner.as_deref().map(|d| &d.kind),
        Some(DatumKind::List {
            delim: Delim::Curly,
            ..
        })
    ));
}

#[test]
fn deref_is_off() {
    // `@x` is an ordinary symbol in EDN, not a Deref prefix.
    let data = edn("@x");
    assert_eq!(data[0].kind, DatumKind::Symbol("@x"));
}

#[test]
fn var_quote_is_off() {
    // `#'x` is code syntax: it must not read as a VarQuote prefix.
    let parsed = parse("#'x", &Options::edn());
    assert!(!matches!(
        parsed.data.first().map(|d| &d.kind),
        Some(DatumKind::Prefixed {
            prefix: Prefix::VarQuote,
            ..
        })
    ));
}

#[test]
fn hashfn_is_off() {
    // `#(...)` must not read as a HashFn prefix.
    let parsed = parse("#(+ % 1)", &Options::edn());
    assert!(!parsed.data.iter().any(|d| matches!(
        d.kind,
        DatumKind::Prefixed {
            prefix: Prefix::HashFn,
            ..
        }
    )));
}

#[test]
fn quote_family_is_off() {
    // `'x`, `` `x ``, `~x`, and `^meta x` are Clojure code syntax, not EDN
    // (ADR-0025): none of them may read as a Prefixed datum.
    for src in ["'x", "`x", "~x", "~@x", "^:kw x"] {
        let parsed = parse(src, &Options::edn());
        assert!(
            !parsed
                .data
                .iter()
                .any(|d| matches!(d.kind, DatumKind::Prefixed { .. })),
            "{src:?} must not read as a reader-macro form: {:?}",
            parsed.data
        );
    }
}

#[test]
fn dialect_maps_to_edn() {
    // Dialect::Edn round-trips through for_dialect.
    let data = parse("@x", &Options::for_dialect(Dialect::Edn)).data;
    assert_eq!(data[0].kind, DatumKind::Symbol("@x"));
}
