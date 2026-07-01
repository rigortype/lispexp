//! Reader tests for the Emacs Lisp dialect.

use sexpp::{parse, Datum, DatumKind, Delim, Options, Prefix};

fn el(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::emacs_lisp());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors: {:?}",
        parsed.errors
    );
    parsed.data
}

#[test]
fn square_brackets_are_vectors() {
    // `[...]` is a data vector in elisp — reported as a Square-delimited list.
    let data = el("[1 2 3]");
    let DatumKind::List { delim, items, .. } = &data[0].kind else {
        panic!("expected vector")
    };
    assert_eq!(*delim, Delim::Square);
    assert_eq!(items.len(), 3);
}

#[test]
fn simple_char_literals() {
    let data = el(r"?a ?A ?0");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Char("?a"),
            &DatumKind::Char("?A"),
            &DatumKind::Char("?0"),
        ]
    );
}

#[test]
fn punctuation_char_literals_do_not_delimit() {
    // `?(`, `?)`, `?;`, `?"` are character literals, not delimiters/comments.
    let data = el(r#"(list ?( ?) ?; ?")"#);
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 5);
    assert_eq!(items[1].kind, DatumKind::Char("?("));
    assert_eq!(items[2].kind, DatumKind::Char("?)"));
    assert_eq!(items[3].kind, DatumKind::Char("?;"));
    assert_eq!(items[4].kind, DatumKind::Char("?\""));
}

#[test]
fn escaped_and_modifier_char_literals() {
    let data = el(r"?\n ?\t ?\C-x ?\M-x ?\^I ?\x41 ?\123");
    assert_eq!(
        data.iter().map(|d| &d.kind).collect::<Vec<_>>(),
        vec![
            &DatumKind::Char(r"?\n"),
            &DatumKind::Char(r"?\t"),
            &DatumKind::Char(r"?\C-x"),
            &DatumKind::Char(r"?\M-x"),
            &DatumKind::Char(r"?\^I"),
            &DatumKind::Char(r"?\x41"),
            &DatumKind::Char(r"?\123"),
        ]
    );
}

#[test]
fn escaped_paren_char_in_a_list() {
    // `?\(` must not open a list.
    let data = el(r"(a ?\( b)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 3);
    assert_eq!(items[1].kind, DatumKind::Char(r"?\("));
}

#[test]
fn function_quote() {
    let data = el("#'ignore");
    assert!(matches!(
        data[0].kind,
        DatumKind::Prefixed {
            prefix: Prefix::FunctionQuote,
            ..
        }
    ));
}

#[test]
fn keywords_and_t_nil() {
    let data = el(":foo t nil");
    assert_eq!(data[0].kind, DatumKind::Keyword(":foo"));
    assert_eq!(data[1].kind, DatumKind::Symbol("t"));
    assert_eq!(data[2].kind, DatumKind::Symbol("nil"));
}

#[test]
fn no_block_comments() {
    // `#|` is not a block comment in elisp; `#` here is just reader syntax and
    // `|...|` is not special either. Only `;` comments exist.
    let data = el("; a comment\n(foo bar)");
    assert_eq!(data.len(), 1);
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 2);
}

#[test]
fn propertized_string_hash_paren() {
    // `#("text" 0 3 (face bold))` — a propertized string literal.
    let data = el(r#"#("ab" 0 1 (face bold))"#);
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected #( hash literal")
    };
    assert_eq!(*tag, "");
    assert!(matches!(
        inner.as_ref().unwrap().kind,
        DatumKind::List { .. }
    ));
}

#[test]
fn bytecode_object() {
    // `#[...]` byte-code object — a hash literal over a bracketed group.
    let data = el("#[257 \"\\300\" [x] 3]");
    let DatumKind::HashLiteral { inner, .. } = &data[0].kind else {
        panic!("expected #[ bytecode object")
    };
    let DatumKind::List { delim, .. } = &inner.as_ref().unwrap().kind else {
        panic!()
    };
    assert_eq!(*delim, Delim::Square);
}

#[test]
fn backquote_in_macro() {
    let data = el("`(if ,test ,@body)");
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
fn dotted_pair_and_radix() {
    let data = el("(a . b) #xFF");
    let DatumKind::List { tail, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(tail.as_ref().unwrap().kind, DatumKind::Symbol("b"));
    assert_eq!(data[1].kind, DatumKind::Number("#xFF"));
}
