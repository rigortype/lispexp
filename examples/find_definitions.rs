//! List the definitions in a snippet, with each one's kind and name.
//!
//! Uses the bundled per-dialect registry and the annotator — no evaluation or
//! macro expansion. Run with:
//!
//! ```sh
//! cargo run --example find_definitions
//! ```

use lispexp::annotate::{annotate_tree, bundled_registry, Role};
use lispexp::{parse, Dialect, Options};

fn main() {
    let source = r#"
(defvar counter 0 "A shared counter.")
(defun increment (n) "Add N to the counter." (setq counter (+ counter n)))
(defmacro with-log (&rest body) `(progn (message "log") ,@body))
(cl-defmethod area ((s square)) (* (side s) (side s)))
"#;

    let parsed = parse(source, &Options::emacs_lisp());
    let registry = bundled_registry(Dialect::EmacsLisp);

    println!("{:<14} {:<12} category", "kind", "name");
    for def in annotate_tree(&parsed.data, &registry) {
        let name = def
            .first(Role::Name)
            .and_then(|d| d.as_symbol())
            .unwrap_or("?");
        println!("{:<14} {:<12} {:?}", def.head, name, def.category);
    }
}
