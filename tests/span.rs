//! Span helper tests.

use lispexp::Span;

#[test]
fn len_and_is_empty() {
    let s = Span::new(3, 7);
    assert_eq!(s.len(), 4);
    assert!(!s.is_empty());

    let empty = Span::new(5, 5);
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
}

#[test]
fn contains_is_half_open() {
    let s = Span::new(3, 7);
    assert!(!s.contains(2));
    assert!(s.contains(3));
    assert!(s.contains(6));
    assert!(!s.contains(7));
}

#[test]
fn into_range() {
    let s = Span::new(2, 9);
    let range: std::ops::Range<usize> = s.into();
    assert_eq!(range, 2usize..9usize);
}
