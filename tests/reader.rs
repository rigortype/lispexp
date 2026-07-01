//! Reader (datum-tree) tests for the Scheme dialect.

use lispexp::{parse, Datum, DatumKind, Delim, Notation, Options, Prefix};

fn scheme(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::scheme());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

fn kinds<'a>(data: &'a [Datum<'a>]) -> Vec<&'a DatumKind<'a>> {
    data.iter().map(|d| &d.kind).collect()
}

#[test]
fn atoms_symbol_and_number() {
    let data = scheme("foo 42 -3.14 .5 list->vector +");
    assert_eq!(
        kinds(&data),
        vec![
            &DatumKind::Symbol("foo"),
            &DatumKind::Number("42"),
            &DatumKind::Number("-3.14"),
            &DatumKind::Number(".5"),
            &DatumKind::Symbol("list->vector"),
            &DatumKind::Symbol("+"),
        ]
    );
}

#[test]
fn strings_and_chars_and_bools() {
    let data = scheme(r#""hi\n" #\a #\space #t #false"#);
    assert_eq!(
        kinds(&data),
        vec![
            &DatumKind::Str("\"hi\\n\""),
            &DatumKind::Char("#\\a"),
            &DatumKind::Char("#\\space"),
            &DatumKind::Bool(true),
            &DatumKind::Bool(false),
        ]
    );
}

#[test]
fn nested_list() {
    let data = scheme("(define (f x) (* x x))");
    assert_eq!(data.len(), 1);
    let DatumKind::List { delim, items, tail } = &data[0].kind else {
        panic!("expected list");
    };
    assert_eq!(*delim, Delim::Round);
    assert!(tail.is_none());
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].kind, DatumKind::Symbol("define"));
}

#[test]
fn square_brackets_are_lists() {
    let data = scheme("(let ([x 1]) x)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    let DatumKind::List { items: binds, .. } = &items[1].kind else {
        panic!("expected bindings list")
    };
    let DatumKind::List { delim, .. } = &binds[0].kind else {
        panic!("expected binding")
    };
    assert_eq!(*delim, Delim::Square);
}

#[test]
fn dotted_pair() {
    let data = scheme("(a . b)");
    let DatumKind::List { items, tail, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DatumKind::Symbol("a"));
    assert_eq!(tail.as_ref().unwrap().kind, DatumKind::Symbol("b"));
}

#[test]
fn quote_shorthand() {
    let data = scheme("'(1 2)");
    let DatumKind::Prefixed {
        prefix,
        notation,
        inner,
        ..
    } = &data[0].kind
    else {
        panic!("expected prefixed")
    };
    assert_eq!(*prefix, Prefix::Quote);
    assert_eq!(*notation, Notation::Shorthand);
    assert!(matches!(inner.kind, DatumKind::List { .. }));
}

#[test]
fn quote_longhand_folds() {
    let data = scheme("(quote x)");
    let DatumKind::Prefixed {
        prefix,
        notation,
        inner,
        ..
    } = &data[0].kind
    else {
        panic!("expected prefixed from longhand fold")
    };
    assert_eq!(*prefix, Prefix::Quote);
    assert_eq!(*notation, Notation::Longhand);
    assert_eq!(inner.kind, DatumKind::Symbol("x"));
}

#[test]
fn quasiquote_unquote_splicing() {
    let data = scheme("`(a ,b ,@c)");
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
fn vector_hash_literal() {
    let data = scheme("#(1 2 3)");
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected hash literal")
    };
    assert_eq!(*tag, "");
    let DatumKind::List { items, .. } = &inner.as_ref().unwrap().kind else {
        panic!()
    };
    assert_eq!(items.len(), 3);
}

#[test]
fn bytevector_tag() {
    let data = scheme("#u8(1 2)");
    let DatumKind::HashLiteral { tag, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*tag, "u8");
}

#[test]
fn line_and_block_and_datum_comments() {
    let src = "; a line comment\n(a #| block #| nested |# still |# b) #;(discarded) c";
    let data = scheme(src);
    // Two top-level forms: the list, and `c` (the #;(...) is discarded).
    assert_eq!(data.len(), 2);
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(
        kinds(items),
        vec![&DatumKind::Symbol("a"), &DatumKind::Symbol("b")]
    );
    assert_eq!(data[1].kind, DatumKind::Symbol("c"));
}

#[test]
fn radix_numbers() {
    let data = scheme("#xFF #b1010 #e1.5");
    assert_eq!(
        kinds(&data),
        vec![
            &DatumKind::Number("#xFF"),
            &DatumKind::Number("#b1010"),
            &DatumKind::Number("#e1.5"),
        ]
    );
}

#[test]
fn piped_symbol() {
    let data = scheme("|hello world|");
    assert_eq!(data[0].kind, DatumKind::Symbol("|hello world|"));
}

#[test]
fn datum_labels() {
    let data = scheme("#0=(a . #0#)");
    let DatumKind::Label { id, inner } = &data[0].kind else {
        panic!("expected label")
    };
    assert_eq!(*id, "0");
    let DatumKind::List { tail, .. } = &inner.kind else {
        panic!()
    };
    assert_eq!(tail.as_ref().unwrap().kind, DatumKind::LabelRef { id: "0" });
}

#[test]
fn unicode_symbol() {
    let data = scheme("(λ (x) x)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items[0].kind, DatumKind::Symbol("λ"));
}

#[test]
fn line_numbers() {
    let data = scheme("a\n(b\n c)\nd");
    assert_eq!(data[0].line, 1); // a
    assert_eq!(data[1].line, 2); // (b ...) starts on line 2
    assert_eq!(data[2].line, 4); // d
}

#[test]
fn hash_vector_inner_quote_not_folded() {
    // `#(quote x)` in Scheme is a two-element vector literal, NOT a folded
    // quote — the hash literal's inner list is data (T3).
    let data = scheme("#(quote x)");
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected hash literal, got {:?}", data[0].kind)
    };
    assert_eq!(*tag, "");
    let inner = inner.as_ref().expect("vector has an inner list");
    let DatumKind::List { items, .. } = &inner.kind else {
        panic!("expected inner list, got {:?}", inner.kind)
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DatumKind::Symbol("quote"));
    assert_eq!(items[1].kind, DatumKind::Symbol("x"));
}

#[test]
fn bool_requires_terminator() {
    // `#thing` is not `#t` + `hing`; without a terminator after `#t`/`#f` it
    // falls through to the hash-atom path (L2).
    let data = scheme("#thing");
    assert_eq!(data.len(), 1, "expected one atom, got {:?}", data);
    assert!(
        !matches!(data[0].kind, DatumKind::Bool(_)),
        "`#thing` must not lex as a boolean: {:?}",
        data[0].kind
    );
    // Properly terminated booleans still work.
    assert_eq!(scheme("#t #f #true #false").len(), 4);
    assert_eq!(scheme("#t")[0].kind, DatumKind::Bool(true));
    assert_eq!(scheme("#false")[0].kind, DatumKind::Bool(false));
}

#[test]
fn srfi4_bytevector_tag_is_one_hash_literal() {
    // SRFI-4 `#f64(1.0 2.0)` — `#f` is not a boolean here (L2); `#f64(` is one
    // hash literal (L3), not Bool + Number + List.
    let data = scheme("#f64(1.0 2.0)");
    assert_eq!(data.len(), 1, "expected one datum, got {:?}", data);
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected hash literal, got {:?}", data[0].kind)
    };
    assert_eq!(*tag, "f64");
    let DatumKind::List { items, .. } = &inner.as_ref().unwrap().kind else {
        panic!("expected inner list")
    };
    assert_eq!(items.len(), 2);
}

#[test]
fn radix_r_number_classifies() {
    // `#36rHELLO` / `#2r1010` are numbers (L3b).
    let data = scheme("#36rHELLO #2r1010");
    assert_eq!(data[0].kind, DatumKind::Number("#36rHELLO"));
    assert_eq!(data[1].kind, DatumKind::Number("#2r1010"));
}

#[test]
fn digit_led_symbols_are_symbols() {
    // `1+`, `1-`, `1x` are the symbols Lisp uses, not numbers (L5); real numbers
    // still classify.
    let data = scheme("1+ 1- 1x 1 1.5 -2/3 1e-5 .5 #xFF");
    let kinds: Vec<_> = data.iter().map(|d| &d.kind).collect();
    assert_eq!(*kinds[0], DatumKind::Symbol("1+"));
    assert_eq!(*kinds[1], DatumKind::Symbol("1-"));
    assert_eq!(*kinds[2], DatumKind::Symbol("1x"));
    assert_eq!(*kinds[3], DatumKind::Number("1"));
    assert_eq!(*kinds[4], DatumKind::Number("1.5"));
    assert_eq!(*kinds[5], DatumKind::Number("-2/3"));
    assert_eq!(*kinds[6], DatumKind::Number("1e-5"));
    assert_eq!(*kinds[7], DatumKind::Number(".5"));
    assert_eq!(*kinds[8], DatumKind::Number("#xFF"));
}
