//! Fault-tolerance tests: malformed input yields a partial tree plus errors,
//! never a panic (ADR-0004).

use lispexp::{parse, DatumKind, Options};

#[test]
fn unclosed_list_recovers() {
    let parsed = parse("(a b", &Options::scheme());
    assert!(!parsed.errors.is_empty(), "expected an unclosed-list error");
    // The partial list is still produced.
    assert_eq!(parsed.data.len(), 1);
    assert!(matches!(parsed.data[0].kind, DatumKind::List { .. }));
}

#[test]
fn stray_close_is_reported_and_skipped() {
    let parsed = parse(") a", &Options::scheme());
    assert!(!parsed.errors.is_empty());
    // Parsing continues past the stray close and reads `a`.
    assert_eq!(parsed.data.last().unwrap().kind, DatumKind::Symbol("a"));
}

#[test]
fn unterminated_string_does_not_panic() {
    let parsed = parse("\"oops", &Options::scheme());
    assert!(!parsed.errors.is_empty());
}

#[test]
fn recovers_to_next_top_level_form() {
    // First form is broken (unclosed), but the second is still read.
    let parsed = parse("(a b) (c", &Options::scheme());
    assert_eq!(parsed.data.len(), 2);
    assert!(!parsed.errors.is_empty());
}

#[test]
fn empty_input() {
    let parsed = parse("", &Options::scheme());
    assert!(parsed.data.is_empty());
    assert!(parsed.errors.is_empty());
}

#[test]
fn dangling_discard_keeps_rest_of_file() {
    // `#;) (a b)`: the discard swallows nothing valid, then a stray close and a
    // real form follow. The rest of the file must not be lost (R1).
    let parsed = parse("#;) (a b)", &Options::scheme());
    assert!(
        parsed
            .data
            .iter()
            .any(|d| matches!(d.kind, DatumKind::List { .. })),
        "the `(a b)` form after a dangling discard must survive: {:?}",
        parsed.data
    );
}

#[test]
fn dangling_quote_keeps_rest_of_file() {
    // `') x`: quote has no operand (a stray close follows), yet `x` must survive.
    let parsed = parse("') x", &Options::scheme());
    assert!(
        parsed.data.iter().any(|d| d.kind == DatumKind::Symbol("x")),
        "`x` after a dangling quote must survive: {:?}",
        parsed.data
    );
}

#[test]
fn dangling_discard_at_eof_is_reported() {
    // `#;` with no operand must report a dangling prefix, not vanish (R1b).
    let parsed = parse("#;", &Options::scheme());
    assert!(!parsed.errors.is_empty(), "`#;` at EOF must be reported");
}

#[test]
fn deep_nesting_never_panics() {
    // 100_000 open parens: must not overflow the stack, must keep going, and
    // report the depth cap exactly once (R2).
    let src = "(".repeat(100_000);
    let parsed = parse(&src, &Options::scheme());
    let depth_errors = parsed
        .errors
        .iter()
        .filter(|e| matches!(e.kind, lispexp::ErrorKind::DepthLimitExceeded))
        .count();
    assert_eq!(depth_errors, 1, "depth cap reported exactly once");
}

#[test]
fn deep_nesting_keeps_prior_siblings() {
    // A shallow sibling before a too-deep form is preserved (R2).
    let src = format!("a {}", "(".repeat(2000));
    let parsed = parse(&src, &Options::scheme());
    assert_eq!(parsed.data[0].kind, DatumKind::Symbol("a"));
}

#[test]
fn balanced_deep_nesting_resyncs() {
    // Balanced deep nesting followed by a sibling: after the cap skips the
    // too-deep region, the trailing sibling is still read (R2 recovery).
    let src = format!("{}{} z", "(".repeat(1000), ")".repeat(1000));
    let parsed = parse(&src, &Options::scheme());
    assert!(
        parsed.data.iter().any(|d| d.kind == DatumKind::Symbol("z")),
        "sibling after a balanced too-deep region must survive: {:?}",
        parsed.data
    );
}

#[test]
fn list_span_contains_children_on_eof() {
    // `(a '`: the unclosed list's span must cover the already-parsed child `a`,
    // not collapse to the open delimiter (R3).
    let parsed = parse("(a '", &Options::scheme());
    let list = &parsed.data[0];
    let DatumKind::List { items, .. } = &list.kind else {
        panic!("expected a list, got {:?}", list.kind)
    };
    let child = &items[0];
    assert!(
        list.span.start <= child.span.start && child.span.end <= list.span.end,
        "list span {:?} must contain child span {:?}",
        list.span,
        child.span
    );
}

#[test]
fn unterminated_string_recovers_next_form() {
    // The unterminated string must not swallow the line-start `(c 1)`. Before
    // the lexer backtrack (R5) the string lexed to EOF and `(c 1)` was lost
    // entirely; now it is recovered into the tree (as a nested form, since the
    // enclosing lists never closed).
    fn contains_c_1(d: &DatumKind<'_>) -> bool {
        match d {
            DatumKind::List { items, .. } => {
                (items.first().map(|i| &i.kind) == Some(&DatumKind::Symbol("c"))
                    && items.get(1).map(|i| &i.kind) == Some(&DatumKind::Number("1")))
                    || items.iter().any(|i| contains_c_1(&i.kind))
            }
            _ => false,
        }
    }
    let parsed = parse("(a (b \"unterminated\n(c 1)", &Options::scheme());
    assert!(
        parsed.data.iter().any(|d| contains_c_1(&d.kind)),
        "`(c 1)` must be recovered, not lost: {:?}",
        parsed.data
    );
}
