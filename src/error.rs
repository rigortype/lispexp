//! Parse diagnostics.

use crate::span::Span;

/// A non-fatal parse diagnostic. The reader is fault-tolerant (ADR-0004): it
/// returns a partial tree plus a list of these, resynchronizing at the next
/// top-level form.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub span: Span,
    /// 1-based line.
    pub line: u32,
    pub message: String,
}
