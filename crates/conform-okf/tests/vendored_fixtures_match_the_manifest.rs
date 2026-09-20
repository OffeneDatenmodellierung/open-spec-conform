//! The other half of the registry's custody chain.
//!
//! `specs.toml`'s `okf` entry pins one file: `SHA256SUMS`, the manifest beside
//! the vendored bundles. `conform-registry` re-hashes that manifest on every
//! run, so the registry cannot drift from it. Nothing in `conform-registry`
//! knows the manifest describes twenty other files, so nothing there would
//! notice a fixture being edited underneath it.
//!
//! This is what notices. Together the two guards close the chain; either one
//! alone leaves a gap you could drive a corpus through.
//!
//! The digest function is `conform_registry::sha256_hex`, the same one the
//! registry hashes with. Deliberately the same: two implementations of SHA-256
//! in one repository is two things to be wrong, and if this one drifted from
//! the registry's the two halves of the chain would be measuring different
//! quantities while both reporting green.

use std::fs;
use std::path::{Path, PathBuf};

use conform_registry::sha256_hex;

mod support;

use support::fixture_root;

/// The manifest as `(digest, bundle-relative path)`, in file order.
fn manifest() -> Vec<(String, String)> {
    let path = fixture_root().join("SHA256SUMS");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (digest, file) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("not a `shasum -a 256` line: {line:?}"));
            (digest.to_owned(), file.to_owned())
        })
        .collect()
}

/// Every vendored markdown file, bundle-relative, sorted.
///
/// `PROVENANCE.md` is excluded: it is this repository's own record *about* the
/// fixtures, not one of the upstream bytes, and hashing it would mean editing
/// the manifest every time somebody clarified a sentence in it.
fn vendored_markdown() -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "md")
                && path.file_name().is_some_and(|name| name != "PROVENANCE.md")
            {
                out.push(path);
            }
        }
    }

    let root = fixture_root();
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut relative: Vec<String> = files
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .expect("under the fixture root")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    relative.sort();
    relative
}

/// Every file the manifest names hashes to what the manifest says.
#[test]
fn every_vendored_file_hashes_to_its_recorded_digest() {
    let root = fixture_root();
    let mut drifted = Vec::new();

    for (expected, file) in manifest() {
        let path = root.join(&file);
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("cannot read {file}: {e}"));
        let actual = sha256_hex(&bytes);
        if actual != expected {
            drifted.push(format!("{file}: manifest {expected}, on disk {actual}"));
        }
    }

    assert!(
        drifted.is_empty(),
        "a vendored upstream fixture no longer hashes to its recorded digest. These bytes are \
         somebody else's published work, pinned to a commit, and the assertions in \
         `upstream_bundles_are_checked_the_same_way` were written against exactly them — a \
         fixture that drifts turns that test from a guard into a description of whatever is \
         there now.\n  {}",
        drifted.join("\n  ")
    );
}

/// The manifest describes the corpus that is actually there, in both
/// directions.
///
/// Without this, deleting a fixture and its manifest line together is a clean
/// run: every remaining digest still matches, and the test above has nothing
/// to say about a file nobody mentioned.
#[test]
fn the_manifest_and_the_corpus_describe_each_other() {
    let listed: Vec<String> = manifest().into_iter().map(|(_, file)| file).collect();
    let on_disk = vendored_markdown();

    assert_eq!(
        listed, on_disk,
        "the manifest and the vendored corpus disagree about which files exist"
    );
    assert_eq!(
        on_disk.len(),
        20,
        "PROVENANCE.md records twenty vendored markdown files across two bundles"
    );
}

/// A negative control, because a comparison that cannot fail is not a check.
///
/// One flipped hex digit in a digest must be caught. The manifest on disk is
/// never touched: the drift is introduced in the copy this test holds.
#[test]
fn a_single_changed_digit_would_be_caught() {
    let root = fixture_root();
    let (recorded, file) = manifest().into_iter().next().expect("a manifest entry");

    let mut tampered = recorded.clone();
    // `0` ↔ `1` on the leading digit: still 64 lower-case hex characters, so
    // nothing but the comparison itself can tell the difference.
    let first = if tampered.starts_with('0') { '1' } else { '0' };
    tampered.replace_range(0..1, &first.to_string());
    assert_ne!(tampered, recorded);

    let bytes = fs::read(root.join(&file)).expect("read the fixture");
    assert_eq!(sha256_hex(&bytes), recorded, "the honest comparison passes");
    assert_ne!(
        sha256_hex(&bytes),
        tampered,
        "and the tampered one does not"
    );
}
