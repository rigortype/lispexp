//! Reader tests for the Common Lisp dialect.

use lispexp::{parse, Datum, DatumKind, Delim, Notation, Options, Prefix};

fn cl(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::common_lisp());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn function_quote() {
    let data = cl("#'car");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!("expected #' function quote")
    };
    assert_eq!(*prefix, Prefix::FunctionQuote);
    assert_eq!(inner.kind, DatumKind::Symbol("car"));
}

#[test]
fn feature_conditional_reads_two_forms() {
    // `#+sbcl (foo)` guards the following form; both the feature test and the
    // guarded form are consumed, yielding a single Prefixed datum.
    let data = cl("#+sbcl (foo) bar");
    assert_eq!(
        data.len(),
        2,
        "feature test + guarded form must be one datum"
    );
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!("expected reader conditional")
    };
    assert_eq!(*prefix, Prefix::ReaderConditional(true));
    assert!(matches!(inner.kind, DatumKind::List { .. }));
    assert_eq!(data[1].kind, DatumKind::Symbol("bar"));
}

#[test]
fn feature_conditional_minus() {
    let data = cl("#-sbcl (foo)");
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::ReaderConditional(false),
            ..
        }
    ));
}

#[test]
fn read_time_eval() {
    let data = cl("#.(+ 1 2)");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*prefix, Prefix::ReadEval);
    assert!(matches!(inner.kind, DatumKind::List { .. }));
}

#[test]
fn keywords_and_packages() {
    let data = cl(":keyword cl-user::foo package:bar");
    assert_eq!(data[0].kind, DatumKind::Keyword(":keyword"));
    // Package-qualified symbols are ordinary symbols, not keywords.
    assert_eq!(data[1].kind, DatumKind::Symbol("cl-user::foo"));
    assert_eq!(data[2].kind, DatumKind::Symbol("package:bar"));
}

#[test]
fn t_and_nil_are_symbols() {
    let data = cl("t nil");
    assert_eq!(data[0].kind, DatumKind::Symbol("t"));
    assert_eq!(data[1].kind, DatumKind::Symbol("nil"));
}

#[test]
fn char_literals() {
    let data = cl(r"#\a #\Space #\Newline");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Char(r"#\a"),
            &DatumKind::Char(r"#\Space"),
            &DatumKind::Char(r"#\Newline"),
        ]
    );
}

#[test]
fn piped_symbol_with_spaces() {
    let data = cl("|hello world|");
    assert_eq!(data[0].kind, DatumKind::Symbol("|hello world|"));
}

#[test]
fn single_escape_in_symbol() {
    // `\(` escapes the paren into the symbol's name.
    let data = cl(r"foo\(bar");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].kind, DatumKind::Symbol(r"foo\(bar"));
}

#[test]
fn brackets_are_ordinary_symbol_chars() {
    // `[` `]` are not delimiters in CL; they are symbol constituents.
    let data = cl("[a]");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].kind, DatumKind::Symbol("[a]"));
}

#[test]
fn backquote_and_vector_and_labels() {
    let bq = cl("`(a ,b ,@c)");
    assert!(matches!(
        bq[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Quasiquote,
            ..
        }
    ));

    let vec = cl("#(1 2 3)");
    let DatumKind::HashLiteral { tag, .. } = &vec[0].kind else {
        panic!("expected vector")
    };
    assert_eq!(*tag, "");

    let labeled = cl("#1=(a . #1#)");
    let DatumKind::Label { id, .. } = &labeled[0].kind else {
        panic!("expected datum label")
    };
    assert_eq!(*id, "1");
}

#[test]
fn dotted_pairs() {
    let data = cl("(a . b)");
    let DatumKind::List { items, tail, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(tail.as_ref().unwrap().kind, DatumKind::Symbol("b"));
}

#[test]
fn block_comment_and_longhand_quote() {
    let data = cl("#| a #| nested |# b |# (quote x)");
    assert_eq!(data.len(), 1);
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::Quote,
            notation: Notation::Longhand,
            ..
        }
    ));
}

#[test]
fn ratios_and_radix_numbers() {
    let data = cl("1/2 #xFF #b1010 3.14d0");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Number("1/2"),
            &DatumKind::Number("#xFF"),
            &DatumKind::Number("#b1010"),
            &DatumKind::Number("3.14d0"),
        ]
    );
}

#[test]
fn round_delimiter_only() {
    let data = cl("(defun f (x) (* x x))");
    let DatumKind::List { delim, items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(*delim, Delim::Round);
    assert_eq!(items.len(), 4);
}
