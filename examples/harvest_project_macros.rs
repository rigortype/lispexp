//! Teach the annotator a project's own def-macros, then annotate uses of them.
//!
//! The spec harvester reads a def-macro's own structure — here an Emacs Lisp
//! arglist — to derive where the name/arglist/body sit, so uses of the macro
//! annotate without any hand-written table. Run with:
//!
//! ```sh
//! cargo run --example harvest_project_macros
//! ```

use lispexp::annotate::{annotate_form, bundled_registry, harvest_source_for, Role};
use lispexp::{parse, Dialect, Options};

fn main() {
    // A project defines its own definition macros.
    let project_macros = r#"
(defmacro define-widget (name arglist &rest body) `(defun ,name ,arglist ,@body))
(defmacro define-command (name &rest body) `(defun ,name () (interactive) ,@body))
"#;

    // Start from the bundled core, then extend it by harvesting the project.
    let mut registry = bundled_registry(Dialect::EmacsLisp);
    let added = harvest_source_for(project_macros, Dialect::EmacsLisp, &mut registry);
    println!("harvested {added} project macro spec(s)\n");

    // Now uses of those macros annotate like any built-in def-form.
    let usage = r#"
(define-widget button (label) (render label))
(define-command save-buffer-quietly (write-file))
"#;
    let parsed = parse(usage, &Options::emacs_lisp());
    for def in &parsed.data {
        match annotate_form(def, &registry) {
            Some(a) => {
                let name = a
                    .first(Role::Name)
                    .and_then(|d| d.as_symbol())
                    .unwrap_or("?");
                println!(
                    "{:<16} name={name:<24} body forms={}",
                    a.head,
                    a.all(Role::Body).count(),
                );
            }
            None => println!("(not recognized as a definition)"),
        }
    }
}
