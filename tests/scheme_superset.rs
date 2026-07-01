//! Reader tests for the tolerant `.scm` "Scheme superset" (ADR-0027).
//!
//! The superset widens R7RS-small with the non-conflicting reader extensions of
//! the `.scm`-using implementations (Gauche, Mosh, Gambit). Each test pins a
//! token shape that would lose sync — or be split — under strict
//! [`Options::scheme`], and confirms it now parses as a single opaque leaf.

use lispexp::{parse, Datum, DatumKind, Options};

fn sup(src: &str) -> Vec<Datum<'_>> {
    let parsed = parse(src, &Options::scheme_superset());
    assert!(
        parsed.errors.is_empty(),
        "unexpected errors for {src:?}: {:?}",
        parsed.errors
    );
    parsed.data
}

/// The whole `#[...]` / `#/.../` literal is captured verbatim as one `Str` leaf.
fn assert_single_str(src: &str) {
    let data = sup(src);
    assert_eq!(data.len(), 1, "{src:?} should be one datum: {data:?}");
    assert_eq!(data[0].kind, DatumKind::Str(src));
}

#[test]
fn gauche_char_set_literal() {
    assert_single_str(r"#[a-z]");
    assert_single_str(r"#[\(\[\{]"); // raw brackets inside are opaque payload
}

#[test]
fn gauche_char_set_posix_class() {
    // The `]` inside `[:alnum:]` must not close the set.
    assert_single_str("#[[:alnum:]]");
    assert_single_str("#[[:xdigit:][:space:]]");
    assert_single_str("#[[:^alpha:]]"); // negated class
}

#[test]
fn malformed_posix_class_does_not_swallow_the_rest() {
    // A malformed `[:` must be bounded (Gauche caps the class name and requires
    // `:]`); the `[` degrades to an ordinary member so the following forms
    // survive (with only a local error for the stray `]`), rather than the
    // token eating to the next distant `]` / EOF. Parsed directly since a
    // bounded local diagnostic is expected here.
    let parsed = parse("#[[:]] (a)", &Options::scheme_superset());
    assert!(
        parsed
            .data
            .iter()
            .any(|d| matches!(&d.kind, DatumKind::List { items, .. }
            if items.first().map(|i| &i.kind) == Some(&DatumKind::Symbol("a")))),
        "the trailing (a) must be recovered: {:?}",
        parsed.data
    );
}

#[test]
fn gauche_empty_char_set() {
    // `#[]` is the empty char-set: the `]` right after `[` closes it.
    let data = sup("(char-set-complement #[])");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected list: {data:?}");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].kind, DatumKind::Str("#[]"));
}

#[test]
fn gauche_char_set_escaped_bracket() {
    // A literal `]` member is written `\]` and does not close the set.
    assert_single_str(r"#[\]]");
}

#[test]
fn regex_literal_slash() {
    assert_single_str("#/[a-z]+/");
    assert_single_str(r##"#/[\\\"]/"##); // escaped delimiters inside
}

#[test]
fn regex_literal_with_flags() {
    assert_single_str("#/abc/i");
}

#[test]
fn regex_consumes_only_the_single_i_flag() {
    // Gauche's read_regexp reads exactly one char after the closing `/` and
    // honors only `i`; a letter-initial token abutting `/` must stay separate.
    let data = sup("(m #/a/x)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected list: {data:?}");
    };
    assert_eq!(
        items.len(),
        3,
        "regex + symbol, not one merged leaf: {items:?}"
    );
    assert_eq!(items[1].kind, DatumKind::Str("#/a/"));
    assert_eq!(items[2].kind, DatumKind::Symbol("x"));
}

#[test]
fn regex_escaped_slash_does_not_close() {
    assert_single_str(r"#/a\/b/");
}

#[test]
fn regex_inside_a_call() {
    let data = sup("(rxmatch #/\\d+/ str)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected list: {data:?}");
    };
    assert_eq!(items.len(), 3);
    assert_eq!(items[1].kind, DatumKind::Str(r"#/\d+/"));
}

#[test]
fn gauche_interpolated_string_is_a_str_leaf() {
    // `#"..."` (string interpolation) lexes like a string leaf.
    let data = sup(r#"#"x is ~(+ 1 2)""#);
    assert_eq!(data.len(), 1);
    assert!(matches!(data[0].kind, DatumKind::Str(_)));
}

#[test]
fn mosh_bytevector_vu8() {
    let data = sup("#vu8(1 2 255)");
    let DatumKind::HashLiteral { tag, inner } = &data[0].kind else {
        panic!("expected hash literal: {data:?}");
    };
    assert_eq!(*tag, "vu8");
    let inner = inner.as_ref().expect("bytevector contents");
    let DatumKind::List { items, .. } = &inner.kind else {
        panic!("expected list inside bytevector");
    };
    assert_eq!(items.len(), 3);
}

#[test]
fn r7rs_bytevector_still_works() {
    let data = sup("#u8(0 1 2)");
    assert!(matches!(data[0].kind, DatumKind::HashLiteral { .. }));
}

#[test]
fn gambit_trailing_colon_keyword() {
    let data = sup("(make-point x: 1 y: 2)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected list: {data:?}");
    };
    assert_eq!(items[1].kind, DatumKind::Keyword("x:"));
    assert_eq!(items[3].kind, DatumKind::Keyword("y:"));
}

#[test]
fn gauche_leading_colon_keyword() {
    let data = sup("(make <point> :x 0 :y 1)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected list: {data:?}");
    };
    assert_eq!(items[2].kind, DatumKind::Keyword(":x"));
    assert_eq!(items[4].kind, DatumKind::Keyword(":y"));
}

#[test]
fn bare_colon_is_a_keyword() {
    // Gauche's own reader reads `:` as a keyword — its test suite asserts
    // `(keyword? (read-from-string ":"))` (tests/symkey.scm). Matches the
    // leading-colon behavior of Common Lisp / ISLisp here.
    let data = sup(":");
    assert_eq!(data[0].kind, DatumKind::Keyword(":"));
}

#[test]
fn trailing_colon_atoms_classify_by_lexical_shape() {
    // Classification is lexical-shape-only (L5): `1:`/`−2:` are not a valid
    // numeric shape (the `:` breaks the numeric body), so they fall to the
    // trailing-colon keyword rule rather than being forced to Number by a bare
    // leading digit. A `#`-radix number keeps its Number shape (`#xFF:`). Either
    // way each stays a single leaf, so there is no reader sync loss.
    let data = sup("1: -2: #xFF:");
    assert_eq!(data[0].kind, DatumKind::Keyword("1:"));
    assert_eq!(data[1].kind, DatumKind::Keyword("-2:"));
    assert_eq!(data[2].kind, DatumKind::Number("#xFF:"));
}

#[test]
fn ordinary_r7rs_is_unchanged() {
    // The widening must not disturb plain R7RS-small.
    let data = sup("(define (square x) (* x x))");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!("expected list");
    };
    assert_eq!(items[0].kind, DatumKind::Symbol("define"));
}

#[test]
fn strict_scheme_still_rejects_superset_syntax() {
    // Proof these are widenings: strict R7RS loses sync on the char-set/regexp
    // shapes (the reader mismatches a delimiter inside the opaque payload).
    let strict = Options::scheme();
    assert!(
        !parse(r"#[\(\[\{]", &strict).errors.is_empty(),
        "strict R7RS should not silently accept a Gauche char-set"
    );
    assert!(
        !parse("#/[)(]/", &strict).errors.is_empty(),
        "strict R7RS should not silently accept a Gauche/Mosh regexp"
    );
}
