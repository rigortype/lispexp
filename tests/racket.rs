//! Reader tests for the Racket dialect.

use lispexp::{parse, Datum, DatumKind, Delim, Options, Parsed, Prefix};

fn rkt(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::racket());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

fn rkt_parsed(src: &str) -> Parsed<'_> {
    parse(src, &Options::racket())
}

#[test]
fn lang_line_is_captured_not_a_datum() {
    let parsed = rkt_parsed("#lang racket/base\n(define x 1)");
    assert_eq!(parsed.lang_line, Some("racket/base"));
    // The `#lang` line is not a datum; only the definition is.
    assert_eq!(parsed.data.len(), 1);
    assert!(matches!(parsed.data[0].kind, DatumKind::List { .. }));
    assert!(parsed.errors.is_empty());
}

#[test]
fn shebang_then_lang() {
    let parsed = rkt_parsed("#!/usr/bin/env racket\n#lang racket\n(+ 1 2)");
    assert_eq!(parsed.lang_line, Some("racket"));
    assert_eq!(parsed.data.len(), 1);
}

#[test]
fn hash_colon_keywords() {
    let data = rkt("(f #:width 10 #:height 20)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items[1].kind, DatumKind::Keyword("#:width"));
    assert_eq!(items[3].kind, DatumKind::Keyword("#:height"));
}

#[test]
fn brackets_and_braces_are_lists() {
    let data = rkt("(let ([x 1] {y 2}) x)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    let DatumKind::List { items: binds, .. } = &items[1].kind else {
        panic!()
    };
    let DatumKind::List { delim: d0, .. } = &binds[0].kind else {
        panic!()
    };
    let DatumKind::List { delim: d1, .. } = &binds[1].kind else {
        panic!()
    };
    assert_eq!(*d0, Delim::Square);
    assert_eq!(*d1, Delim::Curly);
}

#[test]
fn syntax_quote() {
    let data = rkt("#'(lambda (x) x)");
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::VarQuote,
            ..
        }
    ));
}

#[test]
fn vector_forms() {
    // `#(...)`, `#[...]`, `#{...}` are all vector literals.
    let data = rkt("#(1 2) #[3 4] #{5 6}");
    for d in &data {
        let DatumKind::HashLiteral { tag, inner } = &d.kind else {
            panic!("expected vector, got {:?}", d.kind)
        };
        assert_eq!(*tag, "");
        assert!(matches!(
            inner.as_ref().unwrap().kind,
            DatumKind::List { .. }
        ));
    }
}

#[test]
fn booleans_and_chars() {
    let data = rkt(r"#t #f #\a #\newline");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Bool(true),
            &DatumKind::Bool(false),
            &DatumKind::Char(r"#\a"),
            &DatumKind::Char(r"#\newline"),
        ]
    );
}

#[test]
fn datum_comment_and_block_comment() {
    let data = rkt("(a #;b c #| block |# d)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    // `#;b` discards `b`; the block comment is skipped.
    assert_eq!(
        items.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Symbol("a"),
            &DatumKind::Symbol("c"),
            &DatumKind::Symbol("d"),
        ]
    );
}

#[test]
fn no_lang_line_when_absent() {
    let parsed = rkt_parsed("(+ 1 2)");
    assert_eq!(parsed.lang_line, None);
}
