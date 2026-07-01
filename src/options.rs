//! Dialect-configurable reader/lexer settings.
//!
//! [`Options`] is the orthogonal, individually-toggleable syntax configuration
//! the Lexer and Reader share (ADR-0003). A [`Dialect`] is just a named preset
//! constructor. Only Scheme is implemented so far; other presets will layer on
//! these fields as they are built.

/// The role of a bracket pair `[]` or `{}` in a dialect.
///
/// The reader records delimiter *shape* (`Delim`), not meaning, so for the tree
/// only the `Ordinary` distinction (is it a delimiter at all?) affects parsing;
/// `List`/`Vector`/`Map` all mean "an active delimiter" and differ only in the
/// meaning a consumer assigns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimRole {
    List,
    Vector,
    Map,
    /// Not a delimiter — an ordinary symbol-constituent character (e.g. ISLisp).
    Ordinary,
}

impl DelimRole {
    pub fn is_delimiter(self) -> bool {
        self != DelimRole::Ordinary
    }
}

/// A block-comment delimiter pair (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockComment {
    pub open: &'static str,
    pub close: &'static str,
    pub nestable: bool,
}

/// A named dialect. Presets are constructed via [`Options`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Scheme,
}

/// Reader/lexer configuration. Construct via a preset such as
/// [`Options::scheme`], then adjust fields if needed.
#[derive(Debug, Clone)]
pub struct Options {
    /// Character that starts a line comment (`;` for most; `#` for Janet).
    pub line_comment: char,
    /// Block-comment delimiters, if any.
    pub block_comment: Option<BlockComment>,
    /// Whether `#;` discards the next datum.
    pub datum_comment: bool,
    /// Whether `#` introduces reader syntax (`#t`, `#\`, `#(`, ...).
    pub hash_syntax: bool,
    /// Role of `[` `]`.
    pub square: DelimRole,
    /// Role of `{` `}`.
    pub curly: DelimRole,
    /// Whether `#t`/`#f`/`#true`/`#false` are booleans.
    pub booleans: bool,
    /// Whether `#\...` is a character literal.
    pub char_literal: bool,
    /// Whether `|...|` is a piped symbol.
    pub piped_symbols: bool,
    /// Whether `#n=` / `#n#` datum labels are recognized.
    pub datum_labels: bool,
    /// Glyph for `quote` shorthand, if any.
    pub quote: Option<char>,
    /// Glyph for `quasiquote` shorthand, if any.
    pub quasiquote: Option<char>,
    /// Glyph for `unquote` shorthand, if any.
    pub unquote: Option<char>,
    /// Suffix that turns `unquote` into `unquote-splicing` (e.g. `,` + `@`).
    pub splicing_suffix: char,
}

impl Options {
    /// R7RS-small Scheme (the first implemented dialect).
    pub fn scheme() -> Self {
        Options {
            line_comment: ';',
            block_comment: Some(BlockComment {
                open: "#|",
                close: "|#",
                nestable: true,
            }),
            datum_comment: true,
            hash_syntax: true,
            square: DelimRole::List,
            // R7RS reserves `{` `}` for future use; treat as ordinary so the
            // reader neither errors nor invents a meaning.
            curly: DelimRole::Ordinary,
            booleans: true,
            char_literal: true,
            piped_symbols: true,
            datum_labels: true,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
        }
    }

    /// Options for a named [`Dialect`].
    pub fn for_dialect(dialect: Dialect) -> Self {
        match dialect {
            Dialect::Scheme => Options::scheme(),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Options::scheme()
    }
}
