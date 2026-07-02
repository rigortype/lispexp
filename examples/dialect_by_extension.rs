//! Pick a reader preset by file extension, then read.
//!
//! lispexp never infers a dialect across files — the caller chooses one preset
//! per input. Run with:
//!
//! ```sh
//! cargo run --example dialect_by_extension
//! ```

use lispexp::{parse, Dialect, Options};

/// Map a file name to a reader preset. Unknown extensions fall back to Scheme.
fn options_for(path: &str) -> Options {
    match path.rsplit('.').next() {
        Some("clj" | "cljs" | "cljc") => Options::clojure(),
        Some("edn") => Options::edn(),
        Some("el") => Options::emacs_lisp(),
        Some("lisp" | "cl") => Options::common_lisp(),
        Some("rkt") => Options::racket(),
        Some("scm" | "ss") => Options::scheme_superset(),
        _ => Options::for_dialect(Dialect::Scheme),
    }
}

fn main() {
    let inputs = [
        ("square.scm", "(define (square x) (* x x))"),
        ("core.clj", "(defn square [x] (* x x))"),
        ("init.el", "(defun square (x) (* x x))"),
    ];

    for (path, source) in inputs {
        let parsed = parse(source, &options_for(path));
        let head = parsed
            .data
            .first()
            .and_then(|d| d.head_symbol())
            .unwrap_or("?");
        println!(
            "{path:<12} → {} top-level form(s), head `{head}`, {} error(s)",
            parsed.data.len(),
            parsed.errors.len(),
        );
    }
}
