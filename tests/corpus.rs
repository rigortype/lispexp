//! Corpus tests: every source file in a vendored dialect corpus must parse with
//! no errors under that dialect.
//!
//! The corpora are git submodules under `tests/corpus/`. If one is not checked
//! out (e.g. a clone without `git submodule update --init`), its test skips
//! rather than fails, so it is safe in environments without the submodule.

use std::fs;
use std::path::{Path, PathBuf};

use sexpp::{parse, Options};

fn corpus_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(name)
}

fn collect_files(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, exts, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e))
        {
            out.push(path);
        }
    }
}

/// Parse every matching file under `root` with `opts`; assert zero parse errors.
/// `min_files` guards against a hollow-green run (corpus missing or all skipped).
/// `exclude` lists repo-relative paths to skip — reserved for files that use
/// runtime-defined custom reader macros, which a static reader cannot parse.
fn check_corpus(name: &str, exts: &[&str], opts: &Options, min_files: usize, exclude: &[&str]) {
    let root = corpus_dir(name);
    if !root.join(".git").exists() && !root.join("README.md").exists() {
        eprintln!(
            "skipping: corpus `{name}` not checked out at {}\n\
             run `git submodule update --init --depth 1` to enable this test",
            root.display()
        );
        return;
    }

    let mut files = Vec::new();
    collect_files(&root, exts, &mut files);
    files.sort();
    files.retain(|p| {
        let rel = p.strip_prefix(&root).ok().and_then(|r| r.to_str());
        !rel.is_some_and(|r| exclude.contains(&r))
    });
    assert!(
        !files.is_empty(),
        "no source files found under {}",
        root.display()
    );

    let mut failures: Vec<(PathBuf, usize, String)> = Vec::new();
    let mut parsed_count = 0usize;
    let mut skipped: Vec<PathBuf> = Vec::new();

    for path in &files {
        // sexpp reads UTF-8 (`&str`) by contract (ADR-0017); non-UTF-8 files are
        // skipped, but reported.
        let Ok(src) = fs::read_to_string(path) else {
            skipped.push(path.clone());
            continue;
        };
        parsed_count += 1;
        let parsed = parse(&src, opts);
        if !parsed.errors.is_empty() {
            let first = &parsed.errors[0];
            let snippet = src
                .lines()
                .nth(first.line.saturating_sub(1) as usize)
                .unwrap_or("")
                .trim();
            failures.push((
                path.clone(),
                parsed.errors.len(),
                format!("L{} {}: {}", first.line, first.message, snippet),
            ));
        }
    }

    eprintln!(
        "corpus `{name}`: {} files found, {} parsed, {} skipped (non-UTF-8), {} with parse errors",
        files.len(),
        parsed_count,
        skipped.len(),
        failures.len()
    );
    for path in &skipped {
        eprintln!(
            "  skipped: {}",
            path.strip_prefix(&root).unwrap_or(path).display()
        );
    }
    for (path, n, detail) in failures.iter().take(40) {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        eprintln!("  {} ({} errors) — {}", rel.display(), n, detail);
    }

    assert!(
        parsed_count > min_files,
        "only {parsed_count} files parsed — corpus may be missing or mostly skipped"
    );
    assert!(
        failures.is_empty(),
        "{} files in corpus `{name}` failed to parse cleanly",
        failures.len()
    );
}

#[test]
fn chibi_scheme_corpus_parses() {
    check_corpus(
        "chibi-scheme",
        &["scm", "sld"],
        &Options::scheme(),
        500,
        &[],
    );
}

#[test]
fn clojure_corpus_parses() {
    check_corpus(
        "clojure",
        &["clj", "cljc", "cljs"],
        &Options::clojure(),
        100,
        &[],
    );
}

#[test]
fn cl_ppcre_corpus_parses() {
    check_corpus("cl-ppcre", &["lisp"], &Options::common_lisp(), 15, &[]);
}

#[test]
fn magit_corpus_parses() {
    check_corpus("magit", &["el"], &Options::emacs_lisp(), 40, &[]);
}

#[test]
fn lem_corpus_parses() {
    check_corpus(
        "lem",
        &["lisp"],
        &Options::common_lisp(),
        500,
        // Uses a runtime-defined `#?'...'` dispatch macro (vi-mode test DSL),
        // which a static reader cannot parse.
        &["extensions/vi-mode/tests/visual.lisp"],
    );
}
