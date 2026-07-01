//! Tests for structured `ErrorKind` and positioned reparse (ADR-0023).
//!
//! `ErrorKind`'s payload variants are `#[non_exhaustive]`, so consumers (and
//! these tests) pattern-match rather than construct.

use lispexp::{parse, parse_form_at, DatumKind, Delim, ErrorKind, Options};

fn only_error(src: &str) -> ErrorKind {
    let parsed = parse(src, &Options::scheme());
    assert_eq!(parsed.errors.len(), 1, "errors: {:?}", parsed.errors);
    parsed.errors[0].kind.clone()
}

#[test]
fn unclosed_list_kind() {
    assert!(matches!(
        only_error("(a b"),
        ErrorKind::UnclosedList {
            open: Delim::Round,
            ..
        }
    ));
}

#[test]
fn unexpected_delimiter_kind() {
    assert!(matches!(
        only_error(")"),
        ErrorKind::UnexpectedDelimiter {
            found: Delim::Round,
            ..
        }
    ));
}

#[test]
fn mismatched_delimiter_carries_expected_and_found() {
    // `[` opened, `)` closed — non-positional payload records both.
    assert!(matches!(
        only_error("[a)"),
        ErrorKind::MismatchedDelimiter {
            expected: Delim::Square,
            found: Delim::Round,
            ..
        }
    ));
}

#[test]
fn malformed_token_carries_text() {
    // An unterminated string is a malformed token whose text is retained —
    // non-positional, so two different defects stay distinguishable.
    let parsed = parse("\"oops", &Options::scheme());
    assert!(parsed.errors.iter().any(
        |e| matches!(&e.kind, ErrorKind::MalformedToken { text, .. } if text.contains("oops"))
    ));
}

#[test]
fn kind_is_shift_stable_and_hashable() {
    // The same defect at two offsets compares equal (no Span in the kind).
    let a = parse("(a", &Options::scheme()).errors[0].kind.clone();
    let b = parse("   (a", &Options::scheme()).errors[0].kind.clone();
    assert_eq!(a, b);
    // Usable as a set key.
    use std::collections::HashSet;
    let set: HashSet<ErrorKind> = [a, b].into_iter().collect();
    assert_eq!(set.len(), 1);
}

#[test]
fn display_renders_human_message() {
    let msg = format!("{}", only_error("(a b"));
    assert!(msg.contains("unclosed"), "{msg}");
}

#[test]
fn parse_form_at_reads_one_form_with_absolute_spans() {
    let src = "(a b) (c d) (e f)";
    let first = parse_form_at(src, 0, &Options::scheme()).unwrap();
    assert_eq!(first.form.span.text(src), "(a b)");
    assert!(first.errors.is_empty());

    // Feed `end` back to read the next form; spans stay absolute.
    let second = parse_form_at(src, first.end, &Options::scheme()).unwrap();
    assert_eq!(second.form.span.text(src), "(c d)");
    assert_eq!(second.form.span.start, 6);
}

#[test]
fn parse_form_at_offset_in_leading_whitespace() {
    let src = "(a b) (c d)";
    // An offset in the whitespace before a form reads that form (at/after).
    let f = parse_form_at(src, 5, &Options::scheme()).unwrap();
    assert_eq!(f.form.span.text(src), "(c d)");
}

#[test]
fn parse_form_at_reports_local_errors() {
    let src = "(a b) (c";
    let f = parse_form_at(src, 6, &Options::scheme()).unwrap();
    assert!(matches!(f.form.kind, DatumKind::List { .. }));
    assert_eq!(f.errors.len(), 1);
    assert!(matches!(
        f.errors[0].kind,
        ErrorKind::UnclosedList {
            open: Delim::Round,
            ..
        }
    ));
}

#[test]
fn parse_form_at_past_end_is_none() {
    let src = "(a b)   ";
    assert!(parse_form_at(src, 5, &Options::scheme()).is_none());
}

#[test]
fn item_after_dotted_tail_is_reported() {
    // `(a . b c)`: a bare item after the dotted tail is malformed in Scheme;
    // every datum is kept but the disturbance is flagged (R4).
    let parsed = parse("(a . b c)", &Options::scheme());
    assert!(
        parsed
            .errors
            .iter()
            .any(|e| matches!(e.kind, ErrorKind::ItemAfterDottedTail)),
        "expected ItemAfterDottedTail, got {:?}",
        parsed.errors
    );
}

#[test]
fn second_dot_is_reported_in_scheme() {
    // `(a . b . c)` — a second dot is malformed in plain Scheme (no infix dot).
    let parsed = parse("(a . b . c)", &Options::scheme());
    assert!(
        parsed
            .errors
            .iter()
            .any(|e| matches!(e.kind, ErrorKind::ItemAfterDottedTail)),
        "expected ItemAfterDottedTail for a second dot, got {:?}",
        parsed.errors
    );
}

#[test]
fn racket_infix_dot_is_not_an_error() {
    // Racket's `(dom . -> . rng)` infix dot is legitimate, not flagged (R4).
    let parsed = parse("(dom . -> . rng)", &Options::racket());
    assert!(
        parsed.errors.is_empty(),
        "Racket infix dot must parse cleanly: {:?}",
        parsed.errors
    );
}

#[test]
fn depth_limit_exceeded_kind() {
    let src = "(".repeat(2000);
    let parsed = parse(&src, &Options::scheme());
    assert!(
        parsed
            .errors
            .iter()
            .any(|e| matches!(e.kind, ErrorKind::DepthLimitExceeded)),
        "expected DepthLimitExceeded, got {:?}",
        parsed.errors
    );
}
