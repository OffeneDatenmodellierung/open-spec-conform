//! NFR-002, enforced rather than requested.
//!
//! The spec says a format-specific string, schema reference or spec-version
//! constant in `conform-core` is "a defect, not a style note". Human review
//! will not hold that line indefinitely across four adapter crates and a
//! parade of future ones; the first plausible-looking `if format == "..."`
//! special case will land in a hurry, on a Friday, with a green CI run. So the
//! line is held here instead.
//!
//! **Scope.** This scans the crate's own sources: `src/**/*.rs`, `Cargo.toml`
//! and `README.md`. It does not scan `tests/` — this file has to spell the
//! forbidden strings out in order to forbid them, and a scanner that failed on
//! its own needle list would be useless. Nothing in `tests/` ships to a
//! consumer or influences the public API, so the boundary that matters is
//! fully covered.

use std::fs;
use std::path::{Path, PathBuf};

/// Names of specific document standards, and of specific serialization
/// formats this crate must never learn about. Case-insensitive.
///
/// Add to this list when a new adapter joins the family; never remove from it.
const FORBIDDEN: &[&str] = &["odcs", "odps", "okf", "lexicon", "yaml", "frontmatter"];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every file the leakage rule applies to.
fn scanned_files() -> Vec<PathBuf> {
    let root = crate_root();
    let mut files = vec![root.join("Cargo.toml"), root.join("README.md")];
    collect_rust_sources(&root.join("src"), &mut files);
    files.sort();
    files
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("cannot read directory entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// `(needle, one-based line number, the line)` for every hit in `haystack`.
fn hits(haystack: &str) -> Vec<(&'static str, usize, String)> {
    let mut found = Vec::new();
    for (index, line) in haystack.lines().enumerate() {
        let lowered = line.to_ascii_lowercase();
        for needle in FORBIDDEN {
            if lowered.contains(needle) {
                found.push((*needle, index + 1, line.trim().to_owned()));
            }
        }
    }
    found
}

#[test]
fn no_format_leakage_in_crate_sources() {
    let files = scanned_files();
    let mut leaks = Vec::new();

    for file in &files {
        let contents = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
        for (needle, line, text) in hits(&contents) {
            leaks.push(format!("{}:{line}: {needle:?} in {text:?}", file.display()));
        }
    }

    assert!(
        leaks.is_empty(),
        "conform-core names a specific document or serialization format. This crate's entire \
         value is that it does not know about any one of them, so this is a defect, not a style \
         note (spec NFR-002). Move the knowledge into the adapter crate that owns it.\n  {}",
        leaks.join("\n  ")
    );
}

/// A scanner that cannot find anything is a green light that means nothing.
/// This proves the matcher matches, case-insensitively, mid-word and mid-line.
#[test]
fn the_scanner_actually_detects_leakage() {
    for needle in FORBIDDEN {
        let shouty = needle.to_ascii_uppercase();
        let sample = format!("// a comment mentioning {shouty} in passing\nfn fine() {{}}\n");
        let found = hits(&sample);
        assert_eq!(found.len(), 1, "scanner missed {shouty:?}");
        assert_eq!(found[0].0, *needle);
        assert_eq!(found[0].1, 1);
    }

    assert!(
        hits("nothing to see here\n").is_empty(),
        "scanner invented a hit"
    );
}

/// Guards against the other way this test could go vacuously green: scanning
/// an empty or wrongly-rooted file set and passing because it read nothing.
#[test]
fn the_scan_actually_covers_the_crate() {
    let files = scanned_files();
    let names: Vec<String> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    for required in ["lib.rs", "Cargo.toml", "README.md"] {
        assert!(
            names.contains(&required.to_owned()),
            "scan did not reach {required}: {names:?}"
        );
    }
    assert!(
        files.len() >= 5,
        "scan reached only {} files, which is fewer than this crate has: {names:?}",
        files.len()
    );
}
