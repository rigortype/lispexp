//! Reader tests for the Phel dialect (essentially Clojure's reader).

use lispexp::{parse, Datum, DatumKind, Delim, Options, Prefix};

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

#[test]
fn semicolon_is_a_symbol_constituent() {
    // Phel's atom grammar admits `;`: `foo;bar` is one symbol, and the quoted
    // `'*_.%;!:+-?` (from Phel's own tests) reads whole rather than being cut at `;`.
    assert_eq!(phel("foo;bar")[0].kind, DatumKind::Symbol("foo;bar"));

    let data = phel("'*_.%;!:+-?");
    let DatumKind::Prefixed { prefix, inner, .. } = &data[0].kind else {
        panic!("expected a quoted form, got {:?}", data[0].kind)
    };
    assert_eq!(*prefix, Prefix::Quote);
    assert_eq!(inner.kind, DatumKind::Symbol("*_.%;!:+-?"));
}

#[test]
fn pipe_paren_is_short_anonymous_function() {
    // `|(+ $ 1)` is Phel's short anonymous function — one HashFn form, exactly
    // like Clojure's `#(+ % 1)`. `(map |(+ $ 1) xs)` must read as three items
    // (`map`, the fn, `xs`), not four with a stray `|` symbol.
    let data = phel("(map |(+ $ 1) xs)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].kind, DatumKind::Symbol("map"));
    assert_eq!(items[2].kind, DatumKind::Symbol("xs"));
    let DatumKind::Prefixed { prefix, inner, .. } = &items[1].kind else {
        panic!("expected a HashFn form, got {:?}", items[1].kind)
    };
    assert_eq!(*prefix, Prefix::HashFn);
    assert!(matches!(inner.kind, DatumKind::List { .. }));
    // The `|` belongs to the anon-fn form, not a sibling: its span covers `|(…)`.
    assert_eq!(
        &"(map |(+ $ 1) xs)"[items[1].span.start as usize..items[1].span.end as usize],
        "|(+ $ 1)"
    );
}

#[test]
fn bare_pipe_is_an_ordinary_symbol() {
    // Only `|(` opens an anon fn; a `|` elsewhere is a symbol constituent (Phel's
    // atom grammar admits `|`), so `|foo` and `a|b` read as whole symbols and
    // never become a HashFn prefix.
    assert_eq!(phel("|foo")[0].kind, DatumKind::Symbol("|foo"));
    assert_eq!(phel("a|b")[0].kind, DatumKind::Symbol("a|b"));
}

#[test]
fn php_fqn_is_one_symbol_not_char_literals() {
    // A PHP fully-qualified name is a single symbol, not a `\`-char literal.
    let data = phel("(php/new \\RuntimeException)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DatumKind::Symbol("php/new"));
    assert_eq!(items[1].kind, DatumKind::Symbol("\\RuntimeException"));

    // A multi-segment FQN stays one symbol too (not three `\`-chars), so the
    // child count is right for a symbol-accurate pass.
    let data = phel("(foo \\Phel\\Lang\\Symbol)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].kind, DatumKind::Symbol("\\Phel\\Lang\\Symbol"));
}

#[test]
fn backslash_char_literals_still_read() {
    // Genuine char literals still lex: a named char, and a single char at a
    // boundary. Phel's guard only diverts the FQN case.
    assert_eq!(phel("\\newline")[0].kind, DatumKind::Char("\\newline"));
    assert_eq!(phel("\\space")[0].kind, DatumKind::Char("\\space"));
    let data = phel("[\\a \\+]");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items[0].kind, DatumKind::Char("\\a"));
    assert_eq!(items[1].kind, DatumKind::Char("\\+"));
}

#[test]
fn semicolon_at_a_token_boundary_still_comments() {
    // A `;` that begins a token is a line comment, even in Phel.
    let data = phel("(foo ;bar\n baz)");
    let DatumKind::List { items, .. } = &data[0].kind else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DatumKind::Symbol("foo"));
    assert_eq!(items[1].kind, DatumKind::Symbol("baz"));
}
