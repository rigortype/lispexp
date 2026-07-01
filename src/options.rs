//! Dialect-configurable reader/lexer settings.
//!
//! [`Options`] is the orthogonal, individually-toggleable syntax configuration
//! the Lexer and Reader share (ADR-0003). A [`Dialect`] is just a named preset
//! constructor. Scheme, Clojure, Common Lisp, and Emacs Lisp are implemented.

use crate::datum::Prefix;

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

/// How character literals are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharSyntax {
    /// `#\a`, `#\space` (Scheme, Common Lisp).
    HashBackslash,
    /// `\a`, `\newline` (Clojure).
    Backslash,
    /// `?a`, `?\n`, `?\C-x` (Emacs Lisp).
    Question,
}

/// What `#(` means in a dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashParen {
    /// `#(...)` is a vector literal (data) — Scheme.
    Vector,
    /// `#(...)` is an anonymous-function reader macro (code) — Clojure/Phel.
    HashFn,
    /// `#(` is not special.
    None,
}

/// A named dialect. Presets are constructed via [`Options`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Scheme,
    Clojure,
    CommonLisp,
    EmacsLisp,
}

/// Reader/lexer configuration. Construct via a preset such as
/// [`Options::scheme`] or [`Options::clojure`], then adjust fields if needed.
#[derive(Debug, Clone)]
pub struct Options {
    /// Character that starts a line comment (`;` for most; `#` for Janet).
    pub line_comment: char,
    /// Whether a comma is whitespace (Clojure/Phel).
    pub comma_is_whitespace: bool,
    /// Block-comment delimiters, if any.
    pub block_comment: Option<BlockComment>,
    /// Whether `#;` discards the next datum (Scheme).
    pub datum_comment: bool,
    /// Whether `#_` discards the next datum (Clojure/Phel).
    pub discard_underscore: bool,
    /// Whether `#` introduces reader syntax (`#t`, `#\`, `#(`, ...).
    pub hash_syntax: bool,
    /// Role of `[` `]`.
    pub square: DelimRole,
    /// Role of `{` `}`.
    pub curly: DelimRole,
    /// Whether `#{` opens a set literal.
    pub set_literal: bool,
    /// Whether `#"..."` is a regex literal (lexed as a string leaf).
    pub regex_literal: bool,
    /// Whether `#tag <form>` is a tagged literal (Clojure `#inst`, `#uuid`, ...).
    pub tagged_literals: bool,
    /// The prefix `#'` maps to, if any (Clojure `VarQuote`, Common Lisp
    /// `FunctionQuote`).
    pub hash_apostrophe: Option<Prefix>,
    /// Whether `#?`/`#?@` are reader conditionals wrapping the next list (Clojure).
    pub reader_conditional: bool,
    /// Whether `#+`/`#-` are feature conditionals: a feature test followed by a
    /// guarded form (Common Lisp). The reader reads two data.
    pub feature_conditional: bool,
    /// Whether `#.` is a read-time-eval prefix (Common Lisp).
    pub read_eval: bool,
    /// Whether `\` escapes the next character inside a symbol (Common Lisp).
    pub symbol_escape: bool,
    /// Whether `#t`/`#f`/`#true`/`#false` are booleans.
    pub booleans: bool,
    /// How character literals are written, if the dialect has them.
    pub char_syntax: Option<CharSyntax>,
    /// What `#(` means.
    pub hash_paren: HashParen,
    /// Whether `:foo` is a keyword.
    pub keyword_colon: bool,
    /// Whether `|...|` is a piped symbol.
    pub piped_symbols: bool,
    /// Whether `#n=` / `#n#` datum labels are recognized.
    pub datum_labels: bool,
    /// Whether a lone `.` inside a list marks a dotted/improper tail `(a . b)`.
    /// False for Clojure, where `.` is an ordinary interop symbol.
    pub dotted_pairs: bool,
    /// Glyph for `quote` shorthand, if any.
    pub quote: Option<char>,
    /// Glyph for `quasiquote` shorthand, if any.
    pub quasiquote: Option<char>,
    /// Glyph for `unquote` shorthand, if any.
    pub unquote: Option<char>,
    /// Suffix that turns `unquote` into `unquote-splicing` (e.g. `,` + `@`).
    pub splicing_suffix: char,
    /// Glyph for a deref prefix (Clojure `@`), if any.
    pub deref: Option<char>,
    /// Glyph for a metadata prefix (Clojure `^`), if any.
    pub meta: Option<char>,
}

impl Options {
    /// R7RS-small Scheme (the first implemented dialect).
    pub fn scheme() -> Self {
        Options {
            line_comment: ';',
            comma_is_whitespace: false,
            block_comment: Some(BlockComment {
                open: "#|",
                close: "|#",
                nestable: true,
            }),
            datum_comment: true,
            discard_underscore: false,
            hash_syntax: true,
            square: DelimRole::List,
            // R7RS reserves `{` `}` for future use; treat as ordinary so the
            // reader neither errors nor invents a meaning.
            curly: DelimRole::Ordinary,
            set_literal: false,
            regex_literal: false,
            tagged_literals: false,
            hash_apostrophe: None,
            reader_conditional: false,
            feature_conditional: false,
            read_eval: false,
            symbol_escape: false,
            booleans: true,
            char_syntax: Some(CharSyntax::HashBackslash),
            hash_paren: HashParen::Vector,
            keyword_colon: false,
            piped_symbols: true,
            datum_labels: true,
            dotted_pairs: true,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
        }
    }

    /// Clojure.
    pub fn clojure() -> Self {
        Options {
            line_comment: ';',
            comma_is_whitespace: true,
            block_comment: None,
            datum_comment: false,
            discard_underscore: true,
            hash_syntax: true,
            square: DelimRole::Vector,
            curly: DelimRole::Map,
            set_literal: true,
            regex_literal: true,
            tagged_literals: true,
            hash_apostrophe: Some(Prefix::VarQuote),
            reader_conditional: true,
            feature_conditional: false,
            read_eval: false,
            symbol_escape: false,
            booleans: false, // true/false/nil are ordinary symbols
            char_syntax: Some(CharSyntax::Backslash),
            hash_paren: HashParen::HashFn,
            keyword_colon: true,
            piped_symbols: false,
            datum_labels: false,
            dotted_pairs: false,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some('~'),
            splicing_suffix: '@',
            deref: Some('@'),
            meta: Some('^'),
        }
    }

    /// Common Lisp (ANSI).
    pub fn common_lisp() -> Self {
        Options {
            line_comment: ';',
            comma_is_whitespace: false,
            block_comment: Some(BlockComment {
                open: "#|",
                close: "|#",
                nestable: true,
            }),
            datum_comment: false,
            discard_underscore: false,
            hash_syntax: true,
            // `[` `]` `{` `}` are not standard delimiters in CL.
            square: DelimRole::Ordinary,
            curly: DelimRole::Ordinary,
            set_literal: false,
            regex_literal: false,
            tagged_literals: false,
            hash_apostrophe: Some(Prefix::FunctionQuote), // #'fn
            reader_conditional: false,
            feature_conditional: true, // #+/#-
            read_eval: true,           // #.
            symbol_escape: true,       // foo\ bar
            booleans: false,           // t / nil are ordinary symbols
            char_syntax: Some(CharSyntax::HashBackslash),
            hash_paren: HashParen::Vector, // #(...)
            keyword_colon: true,           // :keyword
            piped_symbols: true,           // |foo bar|
            datum_labels: true,            // #n= / #n#
            dotted_pairs: true,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
        }
    }

    /// Emacs Lisp.
    pub fn emacs_lisp() -> Self {
        Options {
            line_comment: ';',
            comma_is_whitespace: false,
            block_comment: None, // `;` line comments only
            datum_comment: false,
            discard_underscore: false,
            hash_syntax: true,
            square: DelimRole::Vector, // `[...]` is a data vector
            curly: DelimRole::Ordinary,
            set_literal: false,
            regex_literal: false,
            tagged_literals: false,
            hash_apostrophe: Some(Prefix::FunctionQuote), // #'fn
            reader_conditional: false,
            feature_conditional: false,
            read_eval: false,
            symbol_escape: true,
            booleans: false,                         // t / nil are ordinary symbols
            char_syntax: Some(CharSyntax::Question), // ?a, ?\n, ?\C-x
            hash_paren: HashParen::Vector,           // #("propertized" ...) string
            keyword_colon: true,                     // :keyword
            piped_symbols: false,
            datum_labels: true, // #1= / #1# circular structure
            dotted_pairs: true,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
        }
    }

    /// Options for a named [`Dialect`].
    pub fn for_dialect(dialect: Dialect) -> Self {
        match dialect {
            Dialect::Scheme => Options::scheme(),
            Dialect::Clojure => Options::clojure(),
            Dialect::CommonLisp => Options::common_lisp(),
            Dialect::EmacsLisp => Options::emacs_lisp(),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Options::scheme()
    }
}
