//! A public line/column index over a source string (ADR-0024).
//!
//! [`LineIndex`] maps between byte offsets and 1-based (line, column) positions,
//! independent of the [`crate::Datum`] tree. It is computed once over a `&str`.
//!
//! Conventions (ADR-0024):
//! - **line and column are 1-based** (LSP's 0-based conversion is the consumer's);
//! - **columns are byte offsets** from the start of the line — char/UTF-16
//!   columns are derived by the consumer from [`LineIndex::line_range`] and the
//!   source (ADR-0017);
//! - line breaks are **`\n` and `\r\n` only** — a lone `\r` is not a break;
//! - [`LineIndex::line_range`] returns the line **content only**, excluding the
//!   terminator.

use std::ops::Range;

/// A single line: its start offset and the offset just past its last content
/// byte (i.e. excluding the `\n`/`\r\n` terminator).
#[derive(Debug, Clone, Copy)]
struct Line {
    start: u32,
    end: u32,
}

/// A precomputed line/column index over a source string (ADR-0024).
///
/// Encoding-agnostic: columns are byte offsets. A consumer needing char or
/// UTF-16 columns slices `source[line_start..offset]` via [`Self::line_range`]
/// and counts.
#[derive(Debug, Clone)]
pub struct LineIndex {
    lines: Vec<Line>,
    len: u32,
}

impl LineIndex {
    /// Build an index over `source`.
    pub fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut lines = Vec::new();
        let mut start = 0u32;
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                let mut end = i as u32;
                // Drop a preceding `\r` from the line content (`\r\n` break).
                if end > start && bytes[i - 1] == b'\r' {
                    end -= 1;
                }
                lines.push(Line { start, end });
                start = i as u32 + 1;
            }
        }
        // The final line runs to end-of-source. Always pushed, so an empty
        // source and a trailing-newline source each expose their last line.
        lines.push(Line {
            start,
            end: bytes.len() as u32,
        });
        LineIndex {
            lines,
            len: bytes.len() as u32,
        }
    }

    /// The number of lines. Always at least 1.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Map a byte `offset` to a 1-based `(line, column)`. Columns are byte
    /// offsets from the line start. An offset past end-of-source clamps to the
    /// last position; an offset on a terminator byte maps past the line's
    /// content (on `\r` of a `\r\n`: content length + 1; on the `\n`: + 2).
    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.len);
        // Number of lines whose start is <= offset == the 1-based line.
        let line = self.lines.partition_point(|l| l.start <= offset).max(1);
        let start = self.lines[line - 1].start;
        (line as u32, offset - start + 1)
    }

    /// Map a 1-based `(line, column)` back to a byte offset. `line` and `col`
    /// are clamped to valid ranges: an overflowing `col` clamps to just past
    /// the line's last content byte and never bleeds into a following line.
    pub fn line_col_to_offset(&self, line: u32, col: u32) -> u32 {
        let idx = (line.max(1) as usize).min(self.lines.len()) - 1;
        let line = self.lines[idx];
        (line.start + col.saturating_sub(1)).min(line.end)
    }

    /// The byte range of line `n`'s content (1-based, terminator excluded), or
    /// `None` if `n` is out of range.
    pub fn line_range(&self, n: u32) -> Option<Range<usize>> {
        let idx = (n as usize).checked_sub(1)?;
        let line = self.lines.get(idx)?;
        Some(line.start as usize..line.end as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_has_one_line() {
        let idx = LineIndex::new("");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.offset_to_line_col(0), (1, 1));
        assert_eq!(idx.line_range(1), Some(0..0));
    }

    #[test]
    fn lf_breaks() {
        let idx = LineIndex::new("ab\ncd");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.offset_to_line_col(0), (1, 1)); // 'a'
        assert_eq!(idx.offset_to_line_col(1), (1, 2)); // 'b'
        assert_eq!(idx.offset_to_line_col(2), (1, 3)); // '\n' -> just past 'b'
        assert_eq!(idx.offset_to_line_col(3), (2, 1)); // 'c'
        assert_eq!(idx.offset_to_line_col(4), (2, 2)); // 'd'
        assert_eq!(idx.line_range(1), Some(0..2)); // "ab"
        assert_eq!(idx.line_range(2), Some(3..5)); // "cd"
        assert_eq!(idx.line_range(3), None);
    }

    #[test]
    fn crlf_terminator_excluded() {
        let idx = LineIndex::new("ab\r\ncd");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_range(1), Some(0..2)); // "ab", not "ab\r"
        assert_eq!(idx.line_range(2), Some(4..6)); // "cd"
        assert_eq!(idx.offset_to_line_col(2), (1, 3)); // '\r' -> just past 'b'
        assert_eq!(idx.offset_to_line_col(4), (2, 1)); // 'c'
    }

    #[test]
    fn lone_cr_is_not_a_break() {
        let idx = LineIndex::new("a\rb");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_range(1), Some(0..3)); // "a\rb"
    }

    #[test]
    fn trailing_newline_yields_final_line() {
        let idx = LineIndex::new("ab\n");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.offset_to_line_col(3), (2, 1)); // EOF after '\n'
        assert_eq!(idx.line_range(2), Some(3..3)); // empty final line
    }

    #[test]
    fn offset_past_end_clamps() {
        let idx = LineIndex::new("ab");
        assert_eq!(idx.offset_to_line_col(99), (1, 3));
    }

    #[test]
    fn line_col_round_trips() {
        let src = "one\ntwo\r\nthree";
        let idx = LineIndex::new(src);
        for off in 0..=src.len() as u32 {
            let (l, c) = idx.offset_to_line_col(off);
            // Within a line's content the inverse is exact.
            if let Some(range) = idx.line_range(l) {
                if (off as usize) <= range.end {
                    assert_eq!(idx.line_col_to_offset(l, c), off, "offset {off}");
                }
            }
        }
    }

    #[test]
    fn line_col_to_offset_clamps() {
        let idx = LineIndex::new("ab\ncd");
        assert_eq!(idx.line_col_to_offset(1, 1), 0);
        assert_eq!(idx.line_col_to_offset(2, 1), 3);
        assert_eq!(idx.line_col_to_offset(99, 99), 5); // clamped to last line end
        assert_eq!(idx.line_col_to_offset(0, 0), 0); // clamped up to line 1 col 1
    }

    #[test]
    fn col_overflow_stays_on_its_line() {
        // A column past the line's content clamps within that line — it must
        // never resolve to a byte on a following line.
        let idx = LineIndex::new("ab\ncd");
        let off = idx.line_col_to_offset(1, 10);
        assert_eq!(off, 2); // just past "ab", before the '\n'
        assert_eq!(idx.offset_to_line_col(off).0, 1);

        // Same for a CRLF line: never lands between '\r' and '\n' or beyond.
        let idx = LineIndex::new("ab\r\ncd");
        let off = idx.line_col_to_offset(1, 10);
        assert_eq!(off, 2); // just past "ab", before the "\r\n"
    }
}
