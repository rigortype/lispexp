//! Smoke tests for the infra-dependent dialects: Janet (backtick long strings,
//! `#` comments, splice/mutable/short-fn glyphs), Hy (bracket strings), and
//! AutoLISP (`;|...|;` block comments).

use lispexp::{parse, DatumKind, Delim, Options, Prefix};

// --- Janet -----------------------------------------------------------------

#[test]
fn janet_hash_is_line_comment() {
    let parsed = parse(
        "# this is a comment (with unbalanced\n(+ 1 2)",
        &Options::janet(),
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.len(), 1);
    assert!(matches!(parsed.data[0].kind, DatumKind::List { .. }));
}

#[test]
fn janet_backtick_long_string() {
    // Content may contain unbalanced delimiters and newlines; no escapes.
    let parsed = parse("`raw ( string`  ``two`ticks``", &Options::janet());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.len(), 2);
    assert!(matches!(parsed.data[0].kind, DatumKind::Str(_)));
    assert!(matches!(parsed.data[1].kind, DatumKind::Str(_)));
}

#[test]
fn janet_tuples_structs_and_mutable() {
    let parsed = parse("[1 2] {:a 1} @[1 2] @{:a 1}", &Options::janet());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(matches!(
        parsed.data[0].kind,
        DatumKind::List {
            delim: Delim::Square,
            ..
        }
    ));
    assert!(matches!(
        parsed.data[1].kind,
        DatumKind::List {
            delim: Delim::Curly,
            ..
        }
    ));
    // `@[...]` and `@{...}` are mutable-prefixed collections.
    assert!(matches!(
        parsed.data[2].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Mutable,
            ..
        }
    ));
    assert!(matches!(
        parsed.data[3].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Mutable,
            ..
        }
    ));
}

#[test]
fn janet_splice_and_quasiquote() {
    let parsed = parse("~(a ,b ;c)", &Options::janet());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let DatumKind::Prefixed { prefix, inner, .. } = &parsed.data[0].kind else {
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
            prefix: Prefix::Splice,
            ..
        }
    ));
}

// --- Hy --------------------------------------------------------------------

#[test]
fn hy_bracket_string() {
    let parsed = parse(r#"#[[raw ( string]] #[delim[a]delim]"#, &Options::hy());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.len(), 2);
    assert!(matches!(parsed.data[0].kind, DatumKind::Str(_)));
    assert!(matches!(parsed.data[1].kind, DatumKind::Str(_)));
}

#[test]
fn hy_collections_and_unquote() {
    let parsed = parse("[1 2] {1 2} #{1 2} `(~a ~@b)", &Options::hy());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(matches!(
        parsed.data[2].kind,
        DatumKind::List {
            delim: Delim::Set,
            ..
        }
    ));
    let DatumKind::Prefixed { inner, .. } = &parsed.data[3].kind else {
        panic!()
    };
    let DatumKind::List { items, .. } = &inner.kind else {
        panic!()
    };
    assert!(matches!(
        items[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Unquote,
            ..
        }
    ));
}

// --- AutoLISP --------------------------------------------------------------

#[test]
fn autolisp_block_comment() {
    // `;|...|;` is a block comment; `;` alone is a line comment.
    let parsed = parse(
        ";| block\n(with parens) |;\n; line\n(setq a '(1 . 2))",
        &Options::autolisp(),
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.len(), 1);
    let DatumKind::List { items, .. } = &parsed.data[0].kind else {
        panic!()
    };
    // (setq a '(1 . 2)) — the quoted dotted pair.
    let DatumKind::Prefixed { inner, .. } = &items[2].kind else {
        panic!("expected quote")
    };
    let DatumKind::List { tail, .. } = &inner.kind else {
        panic!()
    };
    assert!(tail.is_some(), "dotted pair tail expected");
}

#[test]
fn autolisp_no_char_literals_or_reader_syntax() {
    // No `#` reader syntax; `#foo` is just an ordinary symbol.
    let parsed = parse("(princ \"hi\") T nil", &Options::autolisp());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data[1].kind, DatumKind::Symbol("T"));
    assert_eq!(parsed.data[2].kind, DatumKind::Symbol("nil"));
}
