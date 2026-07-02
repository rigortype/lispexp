//! Walk only the *code* of a snippet, skipping quoted/quasiquoted data.
//!
//! [`code_nodes`] yields each code node in pre-order, pruning sealed data
//! (a hard `quote`, a hash literal) and descending porous quasiquote templates
//! (so an unquoted form inside one is still reached). Here we use it to list the
//! head symbol of every *call* — every code list — while ignoring anything that
//! is merely quoted data. No evaluation or macro expansion. Run with:
//!
//! ```sh
//! cargo run --example walk_code_nodes
//! ```

use lispexp::{code_nodes, parse, DatumKind, Options};

fn main() {
    let source = r#"
(when (ready? system)
  (log:info "starting")
  (dispatch '(a b c))                  ; '(a b c) is quoted data — skipped
  (retry `(job ,(next-id) pending)))   ; only (next-id) is code in the template
"#;

    let parsed = parse(source, &Options::scheme());

    // Every code list's head symbol — i.e. every operator actually invoked.
    // Quoted `(a b c)` and the quasiquote template's `job`/`pending` never
    // appear; the unquoted `(next-id)` does.
    println!("operators called in code position:");
    for datum in code_nodes(&parsed.data) {
        if let DatumKind::List { items, .. } = &datum.kind {
            if let Some(head) = items.first().and_then(|d| d.as_symbol()) {
                println!("  {head}");
            }
        }
    }
}
