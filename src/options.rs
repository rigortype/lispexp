//! Dialect-configurable reader/lexer settings.
//!
//! [`Options`] is the orthogonal, individually-toggleable syntax configuration
//! the Lexer and Reader share (ADR-0003). A [`Dialect`] is just a named preset
//! constructor. Scheme, Clojure, Common Lisp, Emacs Lisp, Racket, Janet, Hy,
//! AutoLISP, Guile, Phel, Fennel, LFE, and ISLisp are all implemented, plus a
//! tolerant [`Options::scheme_superset`] for arbitrary `.scm` files.

use crate::datum::Prefix;

/// The role of a bracket pair `[]` or `{}` in a dialect.
///
/// The reader records delimiter *shape* (`Delim`), not meaning, so for the tree
/// only the `Ordinary` distinction (is it a delimiter at all?) affects parsing;
/// `List`/`Vector`/`Map` all mean "an active delimiter" and differ only in the
/// meaning a consumer assigns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DelimRole {
    /// An alternate list delimiter (Scheme `[]`).
    List,
    /// A vector literal (Emacs Lisp `[]`).
    Vector,
    /// A map literal (Clojure `{}`).
    Map,
    /// Not a delimiter — an ordinary symbol-constituent character (e.g. ISLisp).
    Ordinary,
}

impl DelimRole {
    /// Whether this role makes the bracket an active delimiter (not `Ordinary`).
    #[must_use]
    pub fn is_delimiter(self) -> bool {
        self != DelimRole::Ordinary
    }
}

/// A block-comment delimiter pair (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockComment {
    /// The opening delimiter (e.g. `#|`).
    pub open: &'static str,
    /// The closing delimiter (e.g. `|#`).
    pub close: &'static str,
    /// Whether the pair nests.
    pub nestable: bool,
}

/// How character literals are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharSyntax {
    /// `#\a`, `#\space` (Scheme, Common Lisp).
    HashBackslash,
    /// `\a`, `\newline` (Clojure).
    Backslash,
    /// `?a`, `?\n`, `?\C-x` (Emacs Lisp).
    Question,
}

/// What `#(` means in a dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashParen {
    /// `#(...)` is a vector literal (data) — Scheme.
    Vector,
    /// `#(...)` is an anonymous-function reader macro (code) — Clojure/Phel.
    HashFn,
    /// `#(` is not special.
    None,
}

/// What `#[` opens in a dialect. The single `#[` dispatch has several
/// mutually-exclusive meanings across dialects; this enum makes the choice
/// explicit (like [`HashParen`] for `#(`) instead of leaving it to the implicit
/// order of competing flags. Distinct from the bare-`[` delimiter role
/// ([`Options::square`]), which still applies when this is [`HashBracket::None`]
/// (e.g. Racket/Emacs `#[...]` hash-vectors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashBracket {
    /// `#[...]` is a Gauche character-set literal, consumed as an opaque leaf.
    CharSet,
    /// `#[DELIM[...]DELIM]` is a Hy bracket string.
    BracketString,
    /// `#[` has no dedicated meaning; falls back to the [`Options::square`] role.
    None,
}

/// The per-dialect table of reader-macro prefix glyphs (ADR-0016).
///
/// Grouped separately from [`Options`] because these fields are exactly the
/// glyph-to-role assignments a dialect makes for its prefix syntax — a
/// cohesive table, not a grab-bag of independent toggles. Build from a base
/// preset such as [`CharRoles::scheme`] or [`CharRoles::clojure`] and override
/// individual glyphs (`CharRoles { unquote: Some('~'), ..CharRoles::scheme()
/// }`).
///
/// `#[non_exhaustive]`: adding a new prefix role's glyph field is not a
/// breaking change.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CharRoles {
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
    /// Glyph for a splice prefix (Janet `;`), if any.
    pub splice: Option<char>,
    /// Glyph for a mutable-marker prefix (Janet `@`), if any.
    pub mutable: Option<char>,
    /// Glyph for a bare short-function prefix (Janet `|`), if any.
    pub short_fn: Option<char>,
}

impl CharRoles {
    /// The Scheme/Lisp-family base table: `'` quote, `` ` `` quasiquote, `,`
    /// unquote with `@` as the splicing suffix, and no deref/meta/splice/
    /// mutable/short-fn glyphs.
    pub fn scheme() -> Self {
        CharRoles {
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
            splice: None,
            mutable: None,
            short_fn: None,
        }
    }

    /// The Clojure-family base table: [`Self::scheme`]'s quote family, `~` for
    /// unquote, `@` deref, and `^` meta.
    pub fn clojure() -> Self {
        CharRoles {
            unquote: Some('~'),
            deref: Some('@'),
            meta: Some('^'),
            ..CharRoles::scheme()
        }
    }
}

/// A named dialect. Presets are constructed via [`Options`].
///
/// `#[non_exhaustive]`: new dialects are added over time, so downstream `match`es
/// must include a wildcard arm; adding a variant is not a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Dialect {
    /// R7RS-small Scheme.
    Scheme,
    /// Clojure.
    Clojure,
    /// ANSI Common Lisp.
    CommonLisp,
    /// Emacs Lisp.
    EmacsLisp,
    /// Racket.
    Racket,
    /// Janet.
    Janet,
    /// Hy.
    Hy,
    /// AutoLISP.
    AutoLisp,
    /// Guile Scheme.
    Guile,
    /// Phel.
    Phel,
    /// Fennel.
    Fennel,
    /// LFE (Lisp Flavoured Erlang).
    Lfe,
    /// ISLisp.
    Islisp,
    /// A tolerant `.scm` "Scheme superset" — R7RS-small widened with the
    /// non-conflicting reader extensions of the `.scm`-using implementations
    /// (Gauche, Mosh, Gambit). See [`Options::scheme_superset`].
    SchemeSuperset,
    /// EDN (a data-only subset of Clojure).
    Edn,
}

impl Dialect {
    /// Every currently-known dialect, in declaration order.
    ///
    /// `Dialect` is `#[non_exhaustive]`: new variants are added over time, so
    /// `ALL` grows across versions. Do not assume today's `ALL` is complete —
    /// match on `Dialect` with a wildcard arm, and treat this slice as "every
    /// dialect this version of lispexp knows about," not a permanently fixed
    /// set.
    pub const ALL: &'static [Dialect] = &[
        Dialect::Scheme,
        Dialect::Clojure,
        Dialect::CommonLisp,
        Dialect::EmacsLisp,
        Dialect::Racket,
        Dialect::Janet,
        Dialect::Hy,
        Dialect::AutoLisp,
        Dialect::Guile,
        Dialect::Phel,
        Dialect::Fennel,
        Dialect::Lfe,
        Dialect::Islisp,
        Dialect::SchemeSuperset,
        Dialect::Edn,
    ];

    /// The preset [`Options`] for this dialect — sugar for
    /// [`Options::for_dialect`].
    #[must_use]
    pub fn options(&self) -> Options {
        Options::for_dialect(*self)
    }
}

impl std::fmt::Display for Dialect {
    /// The dialect's kebab-case name (e.g. `"common-lisp"`), also accepted by
    /// [`Dialect::from_str`](std::str::FromStr::from_str).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Dialect::Scheme => "scheme",
            Dialect::Clojure => "clojure",
            Dialect::CommonLisp => "common-lisp",
            Dialect::EmacsLisp => "emacs-lisp",
            Dialect::Racket => "racket",
            Dialect::Janet => "janet",
            Dialect::Hy => "hy",
            Dialect::AutoLisp => "autolisp",
            Dialect::Guile => "guile",
            Dialect::Phel => "phel",
            Dialect::Fennel => "fennel",
            Dialect::Lfe => "lfe",
            Dialect::Islisp => "islisp",
            Dialect::SchemeSuperset => "scheme-superset",
            Dialect::Edn => "edn",
        };
        f.write_str(name)
    }
}

/// The error returned by [`Dialect`]'s [`FromStr`](std::str::FromStr) impl
/// when the input names no known dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDialectError {
    input: String,
}

impl std::fmt::Display for ParseDialectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown dialect: `{}`", self.input)
    }
}

impl std::error::Error for ParseDialectError {}

impl std::str::FromStr for Dialect {
    type Err = ParseDialectError;

    /// Parse a dialect's kebab-case [`Display`](std::fmt::Display) form (e.g.
    /// `"common-lisp"`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Dialect::ALL
            .iter()
            .copied()
            .find(|d| d.to_string() == s)
            .ok_or_else(|| ParseDialectError {
                input: s.to_owned(),
            })
    }
}

/// Reader/lexer configuration. Construct via a preset such as
/// [`Options::scheme`] or [`Options::clojure`], then adjust fields if needed.
///
/// `#[non_exhaustive]`: build from a preset and tweak fields (`Options {
/// square: DelimRole::List, ..Options::scheme() }`) rather than naming every
/// field, so adding a syntax toggle is not a breaking change.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
    /// Whether `#"..."` is lexed as an opaque string leaf. The payload's
    /// *meaning* is per-dialect — a Clojure/LFE regex, a Gauche interpolated
    /// string — but the reader records only the shape (a `#`-tagged string),
    /// leaving interpretation to the consumer.
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
    /// Whether `#:foo` is a keyword (Racket, Guile).
    pub hash_keyword: bool,
    /// Whether a leading `#lang <name>` line is captured (Racket).
    pub lang_line: bool,
    /// Whether a leading `#!`-line is a shebang comment (Racket scripts).
    pub shebang_line: bool,
    /// Whether `|...|` is a piped symbol.
    pub piped_symbols: bool,
    /// Whether `#n=` / `#n#` datum labels are recognized.
    pub datum_labels: bool,
    /// Whether a lone `.` inside a list marks a dotted/improper tail `(a . b)`.
    /// False for Clojure, where `.` is an ordinary interop symbol.
    pub dotted_pairs: bool,
    /// The dialect's reader-macro prefix glyph table (ADR-0016).
    pub roles: CharRoles,
    /// Whether a run of backticks delimits a long string (Janet).
    pub long_string_backtick: bool,
    /// What `#[` opens: a Gauche char-set, a Hy bracket string, or nothing
    /// (falling back to the [`Self::square`] role). One choice instead of
    /// competing per-meaning flags.
    pub hash_bracket: HashBracket,
    /// Whether `#/.../` (with optional trailing flag letters) is a regexp
    /// literal (Gauche, Mosh), consumed as an opaque leaf up to the next
    /// unescaped `/`.
    pub regex_slash: bool,
    /// Whether `#vu8(...)` opens a bytevector (R6RS spelling; Mosh). The R7RS
    /// `#u8(...)` spelling is governed by [`HashParen::Vector`] independently.
    pub bytevector_vu8: bool,
    /// Whether a token ending in `:` is a keyword (`foo:` — Gambit/Gerbil,
    /// DSSSL/SRFI-88 style). Distinct from [`Self::keyword_colon`]'s leading
    /// `:foo`.
    pub keyword_trailing_colon: bool,
    /// Whether `(quote x)` and friends are folded into a longhand
    /// [`DatumKind::Prefixed`](crate::DatumKind::Prefixed) (ADR-0002). True for
    /// the Scheme/Lisp family where quote is reader syntax; false for
    /// Clojure/EDN/Janet/Hy/Fennel, whose longhand spellings differ or where
    /// quote is not reader syntax. A per-family glyph gate applies on top:
    /// `quote` folds only if [`Self::quote`] is set, etc.
    pub fold_longhand: bool,
    /// Whether the longhand fold matches the head symbol case-insensitively
    /// (`(QUOTE X)` is `quote`). True for the case-insensitive readers
    /// (Common Lisp, ISLisp, AutoLISP); false elsewhere (Emacs Lisp is
    /// case-sensitive).
    pub fold_case_insensitive: bool,
    /// Whether `#{sym}#` extended symbols are recognized (Guile, ADR-0016):
    /// `#{foo bar}#` is one verbatim [`Symbol`](crate::DatumKind::Symbol) token.
    /// Mutually exclusive with [`Self::set_literal`] (both claim `#{`).
    pub hash_curly_symbol: bool,
    /// Whether the dialect accepts Racket's *infix dot* convention, where a
    /// second dot in a list (`(dom . -> . rng)`, read as `(-> dom rng)`) is
    /// legitimate rather than malformed. When `false` (Scheme, Common Lisp,
    /// Emacs Lisp, …), an item or dot after the dotted tail is reported as
    /// [`ErrorKind::ItemAfterDottedTail`](crate::ErrorKind::ItemAfterDottedTail)
    /// (R4); the reader keeps every datum either way. Only meaningful when
    /// [`Self::dotted_pairs`] is set.
    pub dotted_pairs_infix: bool,
}

impl Options {
    /// R7RS-small Scheme (the first implemented dialect).
    #[must_use]
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            roles: CharRoles::scheme(),
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
            fold_longhand: true,
            fold_case_insensitive: false,
            hash_curly_symbol: false,
            dotted_pairs_infix: false,
        }
    }

    /// Clojure.
    #[must_use]
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            roles: CharRoles::clojure(),
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
            // Clojure's longhand spellings differ (`clojure.core/quote`) and
            // its `'` is genuine reader syntax; do not fold `(quote x)`.
            fold_longhand: false,
            fold_case_insensitive: false,
            hash_curly_symbol: false,
            dotted_pairs_infix: false,
        }
    }

    /// Common Lisp (ANSI).
    #[must_use]
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            roles: CharRoles::scheme(),
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
            fold_longhand: true,
            // Common Lisp's reader is case-insensitive: `(QUOTE X)` is quote.
            fold_case_insensitive: true,
            hash_curly_symbol: false,
            dotted_pairs_infix: false,
        }
    }

    /// Emacs Lisp.
    #[must_use]
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            roles: CharRoles::scheme(),
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
            // Emacs Lisp's reader IS case-sensitive.
            fold_longhand: true,
            fold_case_insensitive: false,
            hash_curly_symbol: false,
            dotted_pairs_infix: false,
        }
    }

    /// Racket. Layers on the Scheme surface with `#lang`, `#:` keywords, `[]`/`{}`
    /// as code lists, `#'` syntax, and `#[`/`#{` vectors.
    #[must_use]
    pub fn racket() -> Self {
        Options {
            square: DelimRole::List,
            curly: DelimRole::List, // `[]` and `{}` are interchangeable with `()`
            hash_apostrophe: Some(Prefix::VarQuote), // #'syntax
            symbol_escape: true,
            keyword_colon: false, // Racket keywords are `#:foo`, not `:foo`
            hash_keyword: true,
            lang_line: true,
            shebang_line: true,
            dotted_pairs_infix: true, // Racket's `(dom . -> . rng)` infix dot
            ..Options::scheme()
        }
    }

    /// Janet. Note: `#` is the line comment, `;` is splice, `~` is quasiquote.
    #[must_use]
    pub fn janet() -> Self {
        Options {
            line_comment: '#',
            block_comment: None,
            datum_comment: false,
            hash_syntax: false,      // `#` is the comment char, not reader syntax
            square: DelimRole::List, // `[...]` bracketed tuple
            curly: DelimRole::Map,   // `{...}` struct
            booleans: false,
            char_syntax: None,
            hash_paren: HashParen::None,
            keyword_colon: true,
            piped_symbols: false,
            datum_labels: false,
            dotted_pairs: false,
            roles: CharRoles {
                quasiquote: Some('~'),
                unquote: Some(','),
                splice: Some(';'),
                mutable: Some('@'), // `@[]` array, `@{}` table, `@"..."` buffer
                short_fn: Some('|'),
                ..CharRoles::scheme()
            },
            long_string_backtick: true, // `` `...` ``
            fold_longhand: false,       // Janet quote is `quote`/`quasiquote` fns, not folded
            ..Options::scheme()
        }
    }

    /// Hy (a Lisp that compiles to Python).
    #[must_use]
    pub fn hy() -> Self {
        Options {
            block_comment: None,
            datum_comment: false,
            discard_underscore: true, // #_
            square: DelimRole::List,  // `[...]` list
            curly: DelimRole::Map,    // `{...}` dict
            set_literal: true,        // #{}
            tagged_literals: true,    // #foo reader macros, #* #**
            booleans: false,          // True/False/None are symbols
            char_syntax: None,
            hash_paren: HashParen::None,
            keyword_colon: true,
            piped_symbols: false,
            datum_labels: false,
            dotted_pairs: false, // `.` is attribute access
            roles: CharRoles {
                unquote: Some('~'), // Clojure-style unquote
                ..CharRoles::scheme()
            },
            hash_bracket: HashBracket::BracketString, // #[[...]] / #[DELIM[...]DELIM]
            fold_longhand: false,                     // Hy longhand differs; `'` is reader syntax
            ..Options::scheme()
        }
    }

    /// AutoLISP (AutoCAD). Minimal: `'` quote only, `;|...|;` block comments,
    /// no character literals, no reader syntax.
    #[must_use]
    pub fn autolisp() -> Self {
        Options {
            block_comment: Some(BlockComment {
                open: ";|",
                close: "|;",
                nestable: false,
            }),
            datum_comment: false,
            hash_syntax: false,
            square: DelimRole::Ordinary,
            booleans: false, // T / nil are symbols
            char_syntax: None,
            hash_paren: HashParen::None,
            piped_symbols: false,
            datum_labels: false,
            roles: CharRoles {
                quasiquote: None, // no backquote/unquote in AutoLISP
                unquote: None,
                ..CharRoles::scheme()
            },
            fold_case_insensitive: true, // AutoLISP's reader is case-insensitive
            ..Options::scheme()
        }
    }

    /// Guile (a Scheme implementation with extensions).
    #[must_use]
    pub fn guile() -> Self {
        Options {
            hash_keyword: true,                      // #:kw keywords
            hash_apostrophe: Some(Prefix::VarQuote), // #'syntax
            hash_curly_symbol: true,                 // #{foo bar}# extended symbols (ADR-0016)
            ..Options::scheme()
        }
    }

    /// Phel (a Clojure-like Lisp that compiles to PHP).
    #[must_use]
    pub fn phel() -> Self {
        // Phel's reader is essentially Clojure's; #php tagged literals are
        // already covered by tagged_literals.
        Options::clojure()
    }

    /// EDN — a data-only preset layered on [`Options::clojure`] with the
    /// code-only reader syntax turned off (ADR-0025): `#(` anonymous functions,
    /// `#'` var-quote, `#?`/`#?@` reader conditionals, `#"…"` regex literals,
    /// `@` deref, `'` quote, `` ` ``/`~`/`~@` syntax-quote family, and `^`
    /// metadata — none of these is EDN. Tagged elements (`#inst`, `#uuid`, user
    /// tags), `#{}` sets, and `#_` discard stay on, since they are valid EDN.
    /// Namespaced maps (`#:ns{…}`) also read (as a tagged-literal marker on the
    /// following map): they are accepted by `clojure.edn` although absent from
    /// the EDN spec text.
    #[must_use]
    pub fn edn() -> Self {
        Options {
            hash_paren: HashParen::None, // no `#(` anonymous functions
            hash_apostrophe: None,       // no `#'` var-quote
            reader_conditional: false,   // no `#?`/`#?@`
            regex_literal: false,        // no `#"…"` regex
            roles: CharRoles {
                deref: None,      // no `@` deref
                quote: None,      // no `'x` quote
                quasiquote: None, // no `` `x `` syntax-quote
                unquote: None,    // no `~x` / `~@x`
                meta: None,       // no `^meta`
                ..CharRoles::clojure()
            },
            ..Options::clojure()
        }
    }

    /// Fennel (a Lisp that compiles to Lua).
    #[must_use]
    pub fn fennel() -> Self {
        Options {
            block_comment: None,
            datum_comment: false,
            square: DelimRole::List, // [...] sequence
            curly: DelimRole::Map,   // {...} table
            booleans: false,         // true/false/nil are symbols
            char_syntax: None,
            hash_paren: HashParen::HashFn, // #(...) hashfn
            keyword_colon: false,          // :foo is a string; kept as a symbol leaf
            piped_symbols: false,
            datum_labels: false,
            dotted_pairs: false,  // `.` is multi-symbol / method access
            fold_longhand: false, // Fennel quote spellings differ; `'` is reader syntax
            ..Options::scheme()
        }
    }

    /// LFE (Lisp Flavoured Erlang).
    #[must_use]
    pub fn lfe() -> Self {
        Options {
            block_comment: Some(BlockComment {
                open: "#|",
                close: "|#",
                nestable: false, // LFE block comments do not nest
            }),
            regex_literal: true, // #"..." binary strings, lexed as a Str leaf
            hash_apostrophe: Some(Prefix::FunctionQuote), // #'name/arity
            booleans: false,     // 'true / 'false atoms
            datum_labels: false,
            ..Options::scheme()
        }
    }

    /// ISLisp (ISO/IEC 13816).
    #[must_use]
    pub fn islisp() -> Self {
        Options {
            square: DelimRole::Ordinary, // [] {} are ordinary symbol chars
            keyword_colon: true,         // :keyword
            hash_apostrophe: Some(Prefix::FunctionQuote), // #'fn
            booleans: false,             // t / nil are symbols
            datum_labels: false,
            fold_case_insensitive: true, // ISLisp's reader is case-insensitive
            ..Options::scheme()
        }
    }

    /// A tolerant "Scheme superset" for reading arbitrary `.scm` files.
    ///
    /// R7RS-small ([`Options::scheme`]) widened with the non-conflicting reader
    /// extensions of the implementations that share the `.scm` extension —
    /// Gauche, Mosh, and Gambit. Chosen so real R7RS code still parses exactly
    /// as it does under [`Options::scheme`], while the previously-fatal token
    /// shapes (`#[...]` char-sets, `#/.../` regexps) and other superset-only
    /// syntax stop losing reader sync:
    ///
    /// - `#[...]` character-set literals and `#/.../` regexps (Gauche/Mosh) are
    ///   consumed as opaque [`DatumKind::Str`](crate::DatumKind::Str) leaves.
    /// - `#"..."` (Gauche interpolated strings) is lexed as a string leaf.
    /// - `#vu8(...)` bytevectors (R6RS/Mosh) alongside R7RS `#u8(...)`.
    /// - leading-colon `:foo` (Gauche/Guile) and trailing-colon `foo:`
    ///   (Gambit/Gerbil, DSSSL/SRFI-88) keywords.
    ///
    /// These are all *widenings*: each only affects input the strict reader
    /// would have rejected or split, never reclassifying valid R7RS in a way
    /// that changes tree shape (a mis-guessed keyword vs. symbol is still a
    /// leaf, never a sync loss). Gerbil's `.ss`-only `[]`→`(@list …)` /
    /// `{}`→`(@method …)` conventions are out of scope; `{` `}` stay ordinary.
    #[must_use]
    pub fn scheme_superset() -> Self {
        Options {
            hash_bracket: HashBracket::CharSet, // Gauche  #[...]
            regex_slash: true,                  // Gauche/Mosh  #/.../
            regex_literal: true,                // Gauche  #"..." interpolated string (Str leaf)
            bytevector_vu8: true,               // Mosh/R6RS  #vu8(...)
            keyword_colon: true,                // Gauche/Guile  :foo
            keyword_trailing_colon: true,       // Gambit/Gerbil  foo:
            dotted_pairs_infix: true,           // tolerate Gauche/Gambit infix-ish dot chains
            ..Options::scheme()
        }
    }

    /// Options for a named [`Dialect`].
    #[must_use]
    pub fn for_dialect(dialect: Dialect) -> Self {
        match dialect {
            Dialect::Scheme => Options::scheme(),
            Dialect::Clojure => Options::clojure(),
            Dialect::CommonLisp => Options::common_lisp(),
            Dialect::EmacsLisp => Options::emacs_lisp(),
            Dialect::Racket => Options::racket(),
            Dialect::Janet => Options::janet(),
            Dialect::Hy => Options::hy(),
            Dialect::AutoLisp => Options::autolisp(),
            Dialect::Guile => Options::guile(),
            Dialect::Phel => Options::phel(),
            Dialect::Fennel => Options::fennel(),
            Dialect::Lfe => Options::lfe(),
            Dialect::Islisp => Options::islisp(),
            Dialect::SchemeSuperset => Options::scheme_superset(),
            Dialect::Edn => Options::edn(),
        }
    }
}

impl Default for Options {
    /// The default is [`Options::scheme`].
    fn default() -> Self {
        Options::scheme()
    }
}
