//! Locating the things every test in this crate is about.

// Each integration test compiles this module separately, so a helper used by
// one of them is dead code in the others. The alternative — a helper per test
// file — would let the tests drift apart on how they build the validator,
// which is the one thing they must agree on.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use conform_core::{ConformanceReport, GatePolicy, Severity, Validator};
use conform_odcs::{Document, OdcsValidator};

/// The workspace root — the directory `specs.toml` lives in.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-odcs should sit two levels below the workspace root")
        .to_path_buf()
}

/// The fixture corpus directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Every fixture, by path, in a stable order.
pub fn fixtures() -> Vec<PathBuf> {
    let dir = fixtures_dir();
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("cannot read directory entry").path())
        .filter(|path| path.is_file())
        .collect();
    paths.sort();
    paths
}

/// One fixture, loaded as a document named after its file.
pub fn fixture(name: &str) -> Document {
    let path = fixtures_dir().join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    Document::new(name, text)
}

/// The validator, built the way a consumer builds it: through the registry, so
/// the schema's provenance is checked before anything is validated against it.
pub fn validator() -> OdcsValidator {
    let path = workspace_root().join("specs.toml");
    OdcsValidator::from_registry_path(&path).unwrap_or_else(|error| {
        panic!(
            "could not build the validator from {}: {error}",
            path.display()
        )
    })
}

/// This crate's verdict on a document: does it fail under the default policy?
///
/// Deliberately phrased as the *gate* question rather than "is the report
/// empty". A conformant document still collects hygiene warnings and a
/// provenance note, and a test that asked for silence would be asserting the
/// wrong thing.
pub fn fails(validator: &OdcsValidator, document: &Document) -> bool {
    validator
        .validate(document)
        .should_gate(GatePolicy::default())
}

/// Every error in a report, rendered, for an assertion message worth reading.
pub fn errors(report: &ConformanceReport) -> Vec<String> {
    report
        .at_or_above(Severity::Error)
        .map(ToString::to_string)
        .collect()
}
