//! WebAssembly text format (WAT) preset (ADR-0035).
//!
//! WAT is a non-Lisp S-expression format: only the list shape is shared with
//! the Lisp dialects. These tests pin the reader surface — comments, identifiers
//! containing Lisp reader glyphs, WAT number shapes, quoted identifiers, and
//! strings — plus the extension registry.

use lispexp::{parse, Datum, DatumKind, Dialect, Options};

fn wat() -> Options {
    Options::wat()
}

fn kinds<'a>(data: &'a [Datum<'a>]) -> Vec<&'a DatumKind<'a>> {
    data.iter().map(|d| &d.kind).collect()
}

fn symbol_texts<'a>(items: &'a [Datum<'a>]) -> Vec<&'a str> {
    items.iter().filter_map(Datum::as_symbol).collect()
}

#[test]
fn mdn_add_module_parses_cleanly() {
    // The final example from MDN's "Understanding WebAssembly text format".
    let source = r#"
(module
  (func $add (param $lhs i32) (param $rhs i32) (result i32)
    local.get $lhs
    local.get $rhs
    i32.add
  )
  (export "add" (func $add))
)
"#;
    let parsed = parse(source, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.len(), 1);

    let module = &parsed.data[0];
    assert_eq!(module.head_symbol(), Some("module"));
    let items = module.items().unwrap();
    assert_eq!(items[0].as_symbol(), Some("module"));

    let func = &items[1];
    assert_eq!(func.head_symbol(), Some("func"));
    assert_eq!(func.items().unwrap()[1].as_symbol(), Some("$add"));
    // The body is a flat instruction list: `local.get $lhs` is two datums in
    // linear (unfolded) position.
    let body = &items[1].items().unwrap()[5..];
    assert_eq!(
        symbol_texts(body),
        vec!["local.get", "$lhs", "local.get", "$rhs", "i32.add"]
    );

    let export = &items[2];
    assert_eq!(export.head_symbol(), Some("export"));
    assert_eq!(export.items().unwrap()[1].as_str(), Some("\"add\""));
}

#[test]
fn folded_instructions_are_lists() {
    let parsed = parse(
        "(func (result i32) (i32.add (i32.const 1) (i32.const 2)))",
        &wat(),
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let func = &parsed.data[0];
    let add = &func.items().unwrap()[2];
    assert_eq!(add.head_symbol(), Some("i32.add"));
    let args = add.items().unwrap();
    assert_eq!(args.len(), 3);
    assert_eq!(args[1].head_symbol(), Some("i32.const"));
    assert!(matches!(
        args[1].items().unwrap()[1].kind,
        DatumKind::Number("1")
    ));
}

#[test]
fn comments_are_wat_style_and_block_comments_nest() {
    let source = r#"
(; outer (; nested ;) still-comment ;)
(module
  ;; a line comment
  (func)
)
"#;
    let parsed = parse(source, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.len(), 1);
    assert_eq!(parsed.data[0].head_symbol(), Some("module"));
    assert_eq!(
        parsed.data[0].items().unwrap()[1].head_symbol(),
        Some("func")
    );
}

#[test]
fn lisp_reader_glyphs_are_identifier_characters() {
    // `'`, `#`, `|`, and `\` are all WAT idchars, so each runs is one symbol
    // rather than a quote/hash/piped-symbol/escape construct.
    let source = r#"(module $a'b $x#y $p|q $r\s)"#;
    let parsed = parse(source, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(
        symbol_texts(parsed.data[0].items().unwrap()),
        vec!["module", "$a'b", "$x#y", "$p|q", "$r\\s"]
    );
    // No prefix datums: `'` did not become a quote.
    assert!(parsed
        .data
        .iter()
        .all(|d| !matches!(d.kind, DatumKind::Prefixed { .. })));
}

#[test]
fn numbers_follow_the_wat_grammar() {
    let numeric = [
        "0",
        "-0",
        "+42",
        "1_000",
        "0x2A",
        "-0x1",
        "0x1_000",
        "1.5",
        "1.",
        "1e10",
        "-1.5e-3",
        "0x1p3",
        "0x1.8p3",
        "0x1.p3",
        "0x1.ap3",
        "-0x1.8P+3",
        "inf",
        "-inf",
        "nan",
        "nan:0x400000",
        "+nan:0x1_2",
    ];
    let symbols = [
        "i32",
        "funcref",
        "$x",
        "local.get",
        "offset=1024",
        "align=4",
        "nan:canonical",
        "0x1.8",
        "1__0",
        "1_",
        ".5",
        "1f",
    ];

    let mut source = String::new();
    for atom in numeric.iter().chain(symbols.iter()) {
        source.push_str(atom);
        source.push(' ');
    }
    let parsed = parse(&source, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let counted = numeric.len();
    let kinds = kinds(&parsed.data);
    for (atom, kind) in numeric.iter().zip(&kinds[..counted]) {
        assert!(
            matches!(kind, DatumKind::Number(text) if text == atom),
            "{atom:?} should be a Number, got {kind:?}"
        );
    }
    for (atom, kind) in symbols.iter().zip(&kinds[counted..]) {
        assert!(
            matches!(kind, DatumKind::Symbol(text) if text == atom),
            "{atom:?} should be a Symbol, got {kind:?}"
        );
    }
}

#[test]
fn quoted_identifiers_are_one_symbol() {
    let parsed = parse(r#"(func $"has spaces" (result i32))"#, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let func = &parsed.data[0];
    let items = func.items().unwrap();
    assert_eq!(items[1].as_symbol(), Some(r#"$"has spaces""#));
    assert_eq!(items.len(), 3);
}

#[test]
fn strings_keep_wat_escapes_verbatim() {
    let source = r#"(data (i32.const 0) "a\22b\u{1F600}\n\"q\"")"#;
    let parsed = parse(source, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let data = &parsed.data[0];
    let arg = &data.items().unwrap()[2];
    assert_eq!(
        arg.as_str(),
        Some(r#""a\22b\u{1F600}\n\"q\"""#),
        "the raw slice, escapes included"
    );
}

#[test]
fn dialect_registry_and_detection() {
    assert_eq!(Dialect::Wat.to_string(), "wat");
    assert_eq!(Dialect::Wat.options(), Options::wat());
    assert!(Dialect::ALL.contains(&Dialect::Wat));

    assert_eq!(Dialect::from_extension("wat"), &[Dialect::Wat]);
    assert_eq!(Dialect::from_extension(".WAST"), &[Dialect::Wat]);
    assert!(Dialect::Wat.extensions().contains(&"wat"));

    let d = lispexp::detect::detect(Some("add.wat"), "(module (func))");
    assert_eq!(d.dialect, Some(Dialect::Wat));
    assert_eq!(d.confidence, lispexp::detect::Confidence::High);

    // `(module …)` is also Racket; content-only must not guess Wat (ADR-0035).
    assert_eq!(
        lispexp::detect::detect(None, "(module (func))").dialect,
        None
    );
    assert!(Options::wat().line_comment_double);
}

#[test]
fn no_lisp_semantics_leak_in() {
    // Quote-family longhand (`(quote x)`) is not folded; a list is just a list.
    let parsed = parse("(quote x)", &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data[0].head_symbol(), Some("quote"));
    assert!(matches!(parsed.data[0].kind, DatumKind::List { .. }));

    // A bare dot is an ordinary atom, not a dotted-pair separator.
    let parsed = parse("(a . b)", &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let list = &parsed.data[0];
    let (true, Some(tail)) = (list.dot_span().is_none(), list.items()) else {
        panic!("expected a proper list with three atoms, got {list:?}");
    };
    assert_eq!(symbol_texts(tail), vec!["a", ".", "b"]);
}

#[test]
fn lone_semicolon_is_an_atom_not_a_comment() {
    // WAT line comments are `;;`. A stray `;` must not swallow a same-line `)`
    // (that would unbalance the module and lose recovery).
    let parsed = parse("(module (func $f (result i32) i32.const 1; ))", &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data[0].head_symbol(), Some("module"));
    let func = &parsed.data[0].items().unwrap()[1];
    assert_eq!(func.head_symbol(), Some("func"));
    let items = func.items().unwrap();
    assert_eq!(items.last().and_then(Datum::as_symbol), Some(";"));

    // `foo;bar` splits at `;` (not an idchar): `foo` then reserved `;bar`.
    let parsed = parse("foo;bar", &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data[0].as_symbol(), Some("foo"));
    assert_eq!(parsed.data[1].as_symbol(), Some(";bar"));
}

#[test]
fn annotations_stay_in_the_tree_as_lists() {
    // WAT treats `(@id …)` as whitespace; lispexp records the tree as written
    // (ADR-0035), so a formatter still sees the annotation.
    let parsed = parse(r#"(module (@custom "n" "v") (func))"#, &wat());
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let items = parsed.data[0].items().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[1].head_symbol(), Some("@custom"));
    assert_eq!(items[1].items().unwrap()[1].as_str(), Some("\"n\""));
    assert_eq!(items[2].head_symbol(), Some("func"));
}
