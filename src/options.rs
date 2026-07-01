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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// What `#[` opens in a dialect. The single `#[` dispatch has several
/// mutually-exclusive meanings across dialects; this enum makes the choice
/// explicit (like [`HashParen`] for `#(`) instead of leaving it to the implicit
/// order of competing flags. Distinct from the bare-`[` delimiter role
/// ([`Options::square`]), which still applies when this is [`HashBracket::None`]
/// (e.g. Racket/Emacs `#[...]` hash-vectors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashBracket {
    /// `#[...]` is a Gauche character-set literal, consumed as an opaque leaf.
    CharSet,
    /// `#[DELIM[...]DELIM]` is a Hy bracket string.
    BracketString,
    /// `#[` has no dedicated meaning; falls back to the [`Options::square`] role.
    None,
}

/// A named dialect. Presets are constructed via [`Options`].
///
/// `#[non_exhaustive]`: new dialects are added over time, so downstream `match`es
/// must include a wildcard arm; adding a variant is not a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Reader/lexer configuration. Construct via a preset such as
/// [`Options::scheme`] or [`Options::clojure`], then adjust fields if needed.
///
/// `#[non_exhaustive]`: build from a preset and tweak fields (`Options {
/// square: DelimRole::List, ..Options::scheme() }`) rather than naming every
/// field, so adding a syntax toggle is not a breaking change.
#[derive(Debug, Clone)]
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
            splice: None,
            mutable: None,
            short_fn: None,
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some('~'),
            splicing_suffix: '@',
            deref: Some('@'),
            meta: Some('^'),
            splice: None,
            mutable: None,
            short_fn: None,
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
            splice: None,
            mutable: None,
            short_fn: None,
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
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
            hash_keyword: false,
            lang_line: false,
            shebang_line: false,
            quote: Some('\''),
            quasiquote: Some('`'),
            unquote: Some(','),
            splicing_suffix: '@',
            deref: None,
            meta: None,
            splice: None,
            mutable: None,
            short_fn: None,
            long_string_backtick: false,
            hash_bracket: HashBracket::None,
            regex_slash: false,
            bytevector_vu8: false,
            keyword_trailing_colon: false,
        }
    }

    /// Racket. Layers on the Scheme surface with `#lang`, `#:` keywords, `[]`/`{}`
    /// as code lists, `#'` syntax, and `#[`/`#{` vectors.
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
            ..Options::scheme()
        }
    }

    /// Janet. Note: `#` is the line comment, `;` is splice, `~` is quasiquote.
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
            quasiquote: Some('~'),
            unquote: Some(','),
            splice: Some(';'),
            mutable: Some('@'), // `@[]` array, `@{}` table, `@"..."` buffer
            short_fn: Some('|'),
            long_string_backtick: true, // `` `...` ``
            ..Options::scheme()
        }
    }

    /// Hy (a Lisp that compiles to Python).
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
            dotted_pairs: false,                      // `.` is attribute access
            unquote: Some('~'),                       // Clojure-style unquote
            hash_bracket: HashBracket::BracketString, // #[[...]] / #[DELIM[...]DELIM]
            ..Options::scheme()
        }
    }

    /// AutoLISP (AutoCAD). Minimal: `'` quote only, `;|...|;` block comments,
    /// no character literals, no reader syntax.
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
            quasiquote: None, // no backquote/unquote in AutoLISP
            unquote: None,
            ..Options::scheme()
        }
    }

    /// Guile (a Scheme implementation with extensions).
    pub fn guile() -> Self {
        Options {
            hash_keyword: true,                      // #:kw keywords
            hash_apostrophe: Some(Prefix::VarQuote), // #'syntax
            ..Options::scheme()
        }
    }

    /// Phel (a Clojure-like Lisp that compiles to PHP).
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
    pub fn edn() -> Self {
        Options {
            hash_paren: HashParen::None, // no `#(` anonymous functions
            hash_apostrophe: None,       // no `#'` var-quote
            reader_conditional: false,   // no `#?`/`#?@`
            regex_literal: false,        // no `#"…"` regex
            deref: None,                 // no `@` deref
            quote: None,                 // no `'x` quote
            quasiquote: None,            // no `` `x `` syntax-quote
            unquote: None,               // no `~x` / `~@x`
            meta: None,                  // no `^meta`
            ..Options::clojure()
        }
    }

    /// Fennel (a Lisp that compiles to Lua).
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
            dotted_pairs: false, // `.` is multi-symbol / method access
            ..Options::scheme()
        }
    }

    /// LFE (Lisp Flavoured Erlang).
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
    pub fn islisp() -> Self {
        Options {
            square: DelimRole::Ordinary, // [] {} are ordinary symbol chars
            keyword_colon: true,         // :keyword
            hash_apostrophe: Some(Prefix::FunctionQuote), // #'fn
            booleans: false,             // t / nil are symbols
            datum_labels: false,
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
    pub fn scheme_superset() -> Self {
        Options {
            hash_bracket: HashBracket::CharSet, // Gauche  #[...]
            regex_slash: true,                  // Gauche/Mosh  #/.../
            regex_literal: true,                // Gauche  #"..." interpolated string (Str leaf)
            bytevector_vu8: true,               // Mosh/R6RS  #vu8(...)
            keyword_colon: true,                // Gauche/Guile  :foo
            keyword_trailing_colon: true,       // Gambit/Gerbil  foo:
            ..Options::scheme()
        }
    }

    /// Options for a named [`Dialect`].
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
    fn default() -> Self {
        Options::scheme()
    }
}
