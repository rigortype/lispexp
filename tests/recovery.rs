//! Fault-tolerance tests: malformed input yields a partial tree plus errors,
//! never a panic (ADR-0004).

use sexpp::{parse, DatumKind, Options};

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
