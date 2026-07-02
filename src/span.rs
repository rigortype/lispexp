//! Byte-range source positions.

/// A byte range into the source string: `[start, end)`.
///
/// Positions are stored as byte offsets (ADR-0008); a 1-based line is attached
/// to each [`crate::Datum`] separately, and column is derived on demand.
///
/// **Input size contract:** offsets are `u32`, so source text over 4 GiB is
/// out of scope — a byte offset past `u32::MAX` cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl Span {
    /// Construct a span from a start (inclusive) and end (exclusive) byte offset.
    #[must_use]
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }

    /// The source text this span covers. Zero-copy.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }

    /// The span's length in bytes.
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Whether the span is empty (`start == end`).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether `offset` falls within this span (`start <= offset < end`).
    pub fn contains(&self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }
}

impl From<Span> for std::ops::Range<usize> {
    fn from(span: Span) -> Self {
        span.start as usize..span.end as usize
    }
}
