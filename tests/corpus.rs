//! Corpus test: every `.scm` / `.sld` file in the chibi-scheme submodule must
//! parse with no errors under the Scheme dialect.
//!
//! The corpus is a git submodule at `tests/corpus/chibi-scheme`. If it is not
//! checked out (e.g. a clone without `git submodule update --init`), the test
//! skips rather than fails, so it is safe in environments without the submodule.

use std::fs;
use std::path::{Path, PathBuf};

use sexpp::{parse, Options};

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/chibi-scheme")
}

fn collect_scheme_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_scheme_files(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("scm") | Some("sld")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn chibi_scheme_corpus_parses() {
    let root = corpus_root();
    if !root.join(".git").exists() && !root.join("lib").exists() {
        eprintln!(
            "skipping: chibi-scheme submodule not checked out at {}\n\
             run `git submodule update --init --depth 1` to enable this test",
            root.display()
        );
        return;
    }

    let mut files = Vec::new();
    collect_scheme_files(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no .scm/.sld files found under {}",
        root.display()
    );

    let opts = Options::scheme();
    let mut failures: Vec<(PathBuf, usize, String)> = Vec::new();
    let mut parsed_count = 0usize;
    let mut skipped: Vec<PathBuf> = Vec::new();

    for path in &files {
        // sexpp reads UTF-8 (`&str`) by contract; non-UTF-8 files (e.g. the
        // deliberately-encoded unicode tests) are skipped, but reported.
        let Ok(src) = fs::read_to_string(path) else {
            skipped.push(path.clone());
            continue;
        };
        parsed_count += 1;
        let parsed = parse(&src, &opts);
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
        "chibi-scheme corpus: {} files found, {} parsed, {} skipped (non-UTF-8), {} with parse errors",
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

    // Guard against a hollow-green run (e.g. everything silently skipped).
    assert!(
        parsed_count > 500,
        "only {} files parsed — corpus may be missing or mostly skipped",
        parsed_count
    );
    assert!(
        failures.is_empty(),
        "{} corpus files failed to parse cleanly",
        failures.len()
    );
}
