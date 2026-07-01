//! Tests for structured `ErrorKind` and positioned reparse (ADR-0023).

use lispexp::{parse, parse_form_at, DatumKind, Delim, ErrorKind, Options};

fn only_error(src: &str) -> ErrorKind {
    let parsed = parse(src, &Options::scheme());
    assert_eq!(parsed.errors.len(), 1, "errors: {:?}", parsed.errors);
    parsed.errors[0].kind.clone()
}

#[test]
fn unclosed_list_kind() {
    assert_eq!(
        only_error("(a b"),
        ErrorKind::UnclosedList { open: Delim::Round }
    );
}

#[test]
fn unexpected_delimiter_kind() {
    assert_eq!(
        only_error(")"),
        ErrorKind::UnexpectedDelimiter {
            found: Delim::Round
        }
    );
}

#[test]
fn mismatched_delimiter_carries_expected_and_found() {
    // `[` opened, `)` closed — non-positional payload records both.
    assert_eq!(
        only_error("[a)"),
        ErrorKind::MismatchedDelimiter {
            expected: Delim::Square,
            found: Delim::Round,
        }
    );
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
    let msg = format!("{}", ErrorKind::UnclosedList { open: Delim::Round });
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
    assert_eq!(
        f.errors.iter().map(|e| &e.kind).collect::<Vec<_>>(),
        vec![&ErrorKind::UnclosedList { open: Delim::Round }]
    );
}

#[test]
fn parse_form_at_past_end_is_none() {
    let src = "(a b)   ";
    assert!(parse_form_at(src, 5, &Options::scheme()).is_none());
}
