//! A recorded digest that is never recomputed is a comfort, not a control.
//!
//! Everything else in the registry is a claim about somewhere else — an
//! upstream repository, a release tag, a licence nobody local can check. The
//! SHA-256 is the one claim that is falsifiable right here, every run, for
//! free. So it is checked here, every run: the registry cannot drift from the
//! bytes it describes, in either direction, without this failing.

mod support;

use std::collections::BTreeSet;
use std::fs;

use conform_core::Severity;
use conform_registry::{Registry, codes, sha256_hex};

#[test]
fn every_entry_hashes_to_its_recorded_digest() {
    let registry = support::real_registry();
    assert!(!registry.entries().is_empty(), "the registry is empty");

    for entry in registry.entries() {
        let path = registry.artefact_path(entry);
        let bytes = fs::read(&path)
            .unwrap_or_else(|e| panic!("`{}` records `{}`: {e}", entry.id, path.display()));

        assert_eq!(
            sha256_hex(&bytes),
            entry.sha256,
            "`{}` ({}, {} bytes) no longer hashes to the digest recorded in specs.toml: \
             either the artefact was edited — in which case it is not the document the \
             entry claims — or specs.toml was updated without re-hashing it",
            entry.id,
            path.display(),
            bytes.len()
        );
    }
}

/// The same check through the crate's own `verify`, which is what a consumer
/// will actually call. Asserted separately so a bug in `verify` cannot hide
/// behind the hand-rolled loop above.
#[test]
fn verify_reports_every_artefact_as_matching() {
    let registry = support::real_registry();
    let report = registry.verify();

    assert_eq!(
        report.len(),
        registry.entries().len(),
        "verify must say something about every entry, including the ones that pass"
    );
    assert_eq!(
        report.count(Severity::Error),
        0,
        "verify found drift:\n  {}",
        report
            .at_or_above(Severity::Error)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    assert!(
        report
            .iter()
            .all(|d| d.code.as_str() == codes::ARTEFACT_VERIFIED)
    );
}

/// The negative control: change one hex digit and `verify` must notice.
#[test]
fn negative_control_a_single_changed_digit_is_caught() {
    let text = support::real_registry_text();
    let recorded = "2cb7dd6fe43344d2233e0406438622681dc3ebadcf8f0d606a15b40c8f6752c0";
    assert!(
        text.contains(recorded),
        "the ODCS digest is no longer spelled `{recorded}`; update this control"
    );

    let sabotaged = text.replacen(
        recorded,
        "3cb7dd6fe43344d2233e0406438622681dc3ebadcf8f0d606a15b40c8f6752c0",
        1,
    );
    let registry = Registry::load_str(&sabotaged, "<specs.toml with one digit changed>")
        .expect("the sabotaged registry parses")
        .with_root(support::workspace_root());

    let report = registry.verify();
    assert_eq!(
        report.count(Severity::Error),
        1,
        "a one-digit change to a recorded digest went unnoticed: {report:?}"
    );
    assert!(
        report
            .iter()
            .any(|d| d.code.as_str() == codes::SHA256_MISMATCH)
    );
}

/// The other negative control: an entry describing bytes that are not there is
/// an error, not a silent pass. A registry entry for a missing file describes
/// nothing.
#[test]
fn negative_control_a_missing_artefact_is_caught() {
    let registry = Registry::load_str(
        r#"
        schema_version = 1

        [[spec]]
        id            = "absent"
        name          = "A Specification Whose Bytes Are Not Here"
        homepage      = "https://example.invalid/absent"
        repository    = "https://example.invalid/absent.git"
        steward       = "Nobody"
        licence       = "Apache-2.0"
        pinned_ref    = "v1.0.0"
        vendored_path = "schemas/this-file-does-not-exist.json"
        sha256        = "0000000000000000000000000000000000000000000000000000000000000000"
        fetched_at    = "2026-09-20"
        "#,
        "<absent>",
    )
    .expect("the registry parses")
    .with_root(support::workspace_root());

    let report = registry.verify();
    assert_eq!(report.count(Severity::Error), 1, "{report:?}");
    assert!(
        report
            .iter()
            .any(|d| d.code.as_str() == codes::ARTEFACT_MISSING)
    );
}

/// No vendored schema without provenance.
///
/// The failure that started all of this was not a wrong entry in a registry;
/// it was a file sitting in a directory with nothing describing it at all. A
/// registry that only describes the artefacts somebody remembered to add would
/// have missed it too.
#[test]
fn every_vendored_schema_is_described_by_an_entry() {
    let registry = support::real_registry();
    let schemas = support::workspace_root().join("schemas");

    let described: BTreeSet<String> = registry
        .entries()
        .iter()
        .map(|entry| entry.vendored_path.clone())
        .collect();

    let mut undescribed = Vec::new();
    for file in fs::read_dir(&schemas).expect("schemas/ should exist") {
        let path = file.expect("directory entry").path();
        if !path.is_file() {
            continue;
        }
        let relative = format!(
            "schemas/{}",
            path.file_name().expect("file name").to_string_lossy()
        );
        if !described.contains(&relative) {
            undescribed.push(relative);
        }
    }
    undescribed.sort();

    assert!(
        undescribed.is_empty(),
        "these vendored artefacts have no entry in specs.toml, so nothing records where \
         they came from:\n  {}",
        undescribed.join("\n  ")
    );
}
