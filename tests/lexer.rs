//! Lexer (Layer 1) tests: the token stream tiles the input and surfaces
//! strings and comments as spans — what a parinfer backend needs (ADR-0015).

use lispexp::{lex, Delim, Options, TokenKind, UnterminatedKind};

#[test]
fn tokens_tile_the_input() {
    let src = "(a ; c\n  \"s\")";
    let opts = Options::scheme();
    let tokens: Vec<_> = lex(src, &opts).collect();

    // Contiguous coverage: each token starts where the previous ended, and the
    // whole input is covered.
    let mut cursor = 0u32;
    for t in &tokens {
        assert_eq!(t.span.start, cursor, "gap or overlap at {:?}", t);
        cursor = t.span.end;
    }
    assert_eq!(cursor as usize, src.len(), "did not cover the whole input");
}

#[test]
fn comments_and_strings_are_surfaced() {
    let src = "a ; comment\n#| block |# \"str\"";
    let opts = Options::scheme();
    let kinds: Vec<TokenKind> = lex(src, &opts).map(|t| t.kind).collect();

    assert!(kinds.contains(&TokenKind::LineComment));
    assert!(kinds.contains(&TokenKind::BlockComment));
    assert!(kinds.contains(&TokenKind::Str));
}

#[test]
fn delimiters_carry_their_shape() {
    let src = "([{";
    // Enable curly as a delimiter for this check by starting from Scheme and
    // flipping the brace role.
    let mut opts = Options::scheme();
    opts.curly = lispexp::DelimRole::List;

    let kinds: Vec<TokenKind> = lex(src, &opts).map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Open(Delim::Round),
            TokenKind::Open(Delim::Square),
            TokenKind::Open(Delim::Curly),
        ]
    );
}

#[test]
fn char_literal_delimiters_do_not_miscount_as_parens() {
    // `#\(` is a character, not an open paren — critical for parinfer.
    let src = r"#\(";
    let opts = Options::scheme();
    let tokens: Vec<_> = lex(src, &opts).collect();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Char);
}

/// Assert that lexing `src` under `opts` yields exactly one token, an
/// [`TokenKind::Unterminated`] of the given `kind`, and that the tiling
/// invariant still holds (the token's span covers the whole input).
fn assert_unterminated(src: &str, opts: &Options, kind: UnterminatedKind) {
    let tokens: Vec<_> = lex(src, opts).collect();
    assert_eq!(tokens.len(), 1, "expected one token, got {tokens:?}");
    assert_eq!(tokens[0].kind, TokenKind::Unterminated(kind));
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(
        tokens[0].span.end as usize,
        src.len(),
        "tiling invariant broken"
    );
}

#[test]
fn unterminated_string_is_str_kind() {
    assert_unterminated(r#""abc"#, &Options::scheme(), UnterminatedKind::Str);
}

#[test]
fn unterminated_piped_symbol_is_piped_symbol_kind() {
    assert_unterminated("|ab", &Options::scheme(), UnterminatedKind::PipedSymbol);
}

#[test]
fn unterminated_guile_extended_symbol_is_piped_symbol_kind() {
    // Guile `#{foo bar}#` extended symbols share the "symbol delimiter pair"
    // category with `|...|` (ADR-0016).
    assert_unterminated("#{ab", &Options::guile(), UnterminatedKind::PipedSymbol);
}

#[test]
fn unterminated_block_comment_is_block_comment_kind_with_depth() {
    // Non-nestable: only ever one level open.
    let mut opts = Options::scheme();
    opts.block_comment = opts.block_comment.map(|bc| lispexp::BlockComment {
        nestable: false,
        ..bc
    });
    assert_unterminated("#| a", &opts, UnterminatedKind::BlockComment { depth: 1 });

    // Nestable: one extra unmatched `#|` deepens the open count.
    assert_unterminated(
        "#| a #| b",
        &Options::scheme(),
        UnterminatedKind::BlockComment { depth: 2 },
    );
}

#[test]
fn unterminated_janet_long_string_is_long_string_kind() {
    assert_unterminated("`abc", &Options::janet(), UnterminatedKind::LongString);
}

#[test]
fn unterminated_hy_bracket_string_is_bracket_string_kind() {
    // Missing the second `[` entirely.
    assert_unterminated("#[[x", &Options::hy(), UnterminatedKind::BracketString);
    // Opener complete but body never closed.
    assert_unterminated("#[[x]y", &Options::hy(), UnterminatedKind::BracketString);
}

#[test]
fn unterminated_char_set_is_char_set_kind() {
    assert_unterminated(
        "#[a-z",
        &Options::scheme_superset(),
        UnterminatedKind::CharSet,
    );
}

#[test]
fn unterminated_regex_slash_is_regex_kind() {
    assert_unterminated("#/re", &Options::scheme_superset(), UnterminatedKind::Regex);
}
