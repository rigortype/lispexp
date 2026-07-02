//! Print the token stream, including the trivia (whitespace, comments) the
//! datum tree drops — the layer a formatter or parinfer backend works from.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example lex_tokens
//! ```

use lispexp::{lex, Options};

fn main() {
    let source = "(define x ; the answer\n  42)";

    for token in lex(source, &Options::scheme()) {
        let text = token.span.text(source);
        // Show newlines/spaces legibly.
        let shown = text.replace('\n', "\\n");
        println!(
            "{:>2}..{:<2} {:<16} {:?}",
            token.span.start,
            token.span.end,
            format!("{:?}", token.kind),
            shown,
        );
    }
}
