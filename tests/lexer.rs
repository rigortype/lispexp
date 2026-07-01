//! Lexer (Layer 1) tests: the token stream tiles the input and surfaces
//! strings and comments as spans — what a parinfer backend needs (ADR-0015).

use sexpp::{lex, Delim, Options, TokenKind};

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
    opts.curly = sexpp::DelimRole::List;

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
