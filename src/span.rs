//! Byte-range source positions.

/// A byte range into the source string: `[start, end)`.
///
/// Positions are stored as byte offsets (ADR-0008); a 1-based line is attached
/// to each [`crate::Datum`] separately, and column is derived on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }

    /// The source text this span covers. Zero-copy.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }
}
