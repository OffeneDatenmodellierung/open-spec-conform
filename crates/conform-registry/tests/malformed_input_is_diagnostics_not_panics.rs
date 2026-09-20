//! A broken registry must be *reported*, in the same shape as everything else.
//!
//! The estate this crate came from reports a failed validation as
//! `Result<(), String>`: one stringly-typed error per document, with no
//! severity, no stable code, no location, and no way to say a second thing.
//! Nothing downstream can filter, group, count or annotate it. This crate
//! refuses to add another of those, so every way of handing it a bad registry
//! is checked here to produce `conform-core` diagnostics — with a code, a
//! severity and a position — and never a panic.

mod support;

use conform_core::{GatePolicy, Severity};
use conform_registry::{LoadError, Registry, codes};

/// Load something that is not a registry, and expect to be told why.
fn load_failure(text: &str) -> LoadError {
    match Registry::load_str(text, "<broken>") {
        Ok(registry) => panic!("a broken registry loaded: {registry:?}"),
        Err(error) => error,
    }
}

#[test]
fn text_that_is_not_toml_reports_a_position() {
    let error = load_failure("schema_version = 1\n\n[[spec]\nid = \"unclosed\"\n");
    let report = error.report();

    assert_eq!(report.len(), 1);
    let diagnostic = &report.diagnostics()[0];
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.code.as_str(), codes::MALFORMED);
    assert_eq!(
        diagnostic.location.line,
        Some(3),
        "a syntax error must name the line it is on: {diagnostic}"
    );
    assert!(diagnostic.location.column.is_some());
    assert!(report.should_gate(GatePolicy::default()));
}

#[test]
fn a_missing_required_field_names_it() {
    let error = load_failure(
        r#"
        schema_version = 1

        [[spec]]
        id   = "no-bytes"
        name = "An entry that forgot the bytes it is about"
        "#,
    );

    let diagnostic = &error.report().diagnostics()[0];
    assert_eq!(diagnostic.code.as_str(), codes::MALFORMED);
    assert!(
        diagnostic.message.contains("vendored_path"),
        "the message should name the missing field: {}",
        diagnostic.message
    );
}

/// The typo that would otherwise be silent. `sha_256` accepted leniently would
/// leave `sha256` missing — a registry entry describing bytes it cannot check,
/// which is the whole failure mode again in miniature.
#[test]
fn a_misspelled_field_is_refused_rather_than_ignored() {
    let error = load_failure(
        r#"
        schema_version = 1

        [[spec]]
        id            = "typo"
        name          = "An entry with a misspelled digest field"
        pinned_ref    = "v1.0.0"
        vendored_path = "schemas/typo.json"
        sha_256       = "0000000000000000000000000000000000000000000000000000000000000000"
        fetched_at    = "2026-09-20"
        "#,
    );

    let diagnostic = &error.report().diagnostics()[0];
    assert!(
        diagnostic.message.contains("sha_256") || diagnostic.message.contains("unknown"),
        "the message should point at the unknown key: {}",
        diagnostic.message
    );
}

#[test]
fn an_unknown_schema_version_is_refused_rather_than_guessed_at() {
    let error = load_failure("schema_version = 2\n");
    let diagnostic = &error.report().diagnostics()[0];

    assert_eq!(diagnostic.code.as_str(), codes::SCHEMA_VERSION);
    assert!(diagnostic.help.is_some());
    assert_eq!(
        diagnostic.location.pointer.as_ref().map(|p| p.as_str()),
        Some("/schema_version")
    );
}

#[test]
fn a_registry_that_is_not_there_is_reported_not_panicked() {
    let missing = support::workspace_root().join("this-registry-does-not-exist.toml");
    let error = Registry::load_path(&missing).expect_err("there is no such file");

    let diagnostic = &error.report().diagnostics()[0];
    assert_eq!(diagnostic.code.as_str(), codes::UNREADABLE);
    assert!(
        diagnostic
            .location
            .document
            .as_str()
            .contains("this-registry-does-not-exist.toml")
    );
}

/// Semantic breakage is reported per-problem, not one-problem-per-document.
/// Four defects in one entry produce four diagnostics: the multi-diagnostic
/// reporting that `Result<(), String>` destroys is the reason `conform-core`
/// exists.
#[test]
fn several_problems_in_one_entry_are_all_reported() {
    let registry = Registry::load_str(
        r#"
        schema_version = 1

        [[spec]]
        id            = "several"
        name          = ""
        homepage      = "http://example.invalid/insecure"
        repository    = "https://example.invalid/several.git"
        steward       = "Nobody"
        licence       = "Apache-2.0"
        pinned_ref    = "HEAD"
        vendored_path = "/etc/passwd"
        sha256        = "not-a-digest"
        fetched_at    = "last August"
        "#,
        "<several>",
    )
    .expect("the file parses; it is the contents that are wrong");

    let report = registry.validate();
    let codes_found: Vec<&str> = report.iter().map(|d| d.code.as_str()).collect();

    for expected in [
        codes::BLANK_FIELD,
        codes::MOVING_REF,
        codes::PATH_NOT_REPO_RELATIVE,
        codes::MALFORMED_SHA256,
        codes::MALFORMED_DATE,
        codes::INSECURE_URL,
    ] {
        assert!(
            codes_found.contains(&expected),
            "{expected} missing from {codes_found:?}"
        );
    }
    assert!(
        report.count(Severity::Error) >= 5,
        "every defect should be reported, not just the first: {report:?}"
    );
}

#[test]
fn two_entries_with_the_same_id_are_reported() {
    let registry = Registry::load_str(
        r#"
        schema_version = 1

        [[spec]]
        id            = "twice"
        name          = "First"
        homepage      = "https://example.invalid/a"
        repository    = "https://example.invalid/a.git"
        steward       = "Nobody"
        licence       = "Apache-2.0"
        pinned_ref    = "v1.0.0"
        vendored_path = "schemas/a.json"
        sha256        = "0000000000000000000000000000000000000000000000000000000000000000"
        fetched_at    = "2026-09-20"

        [[spec]]
        id            = "twice"
        name          = "Second"
        homepage      = "https://example.invalid/b"
        repository    = "https://example.invalid/b.git"
        steward       = "Nobody"
        licence       = "Apache-2.0"
        pinned_ref    = "v2.0.0"
        vendored_path = "schemas/b.json"
        sha256        = "0000000000000000000000000000000000000000000000000000000000000000"
        fetched_at    = "2026-09-20"
        "#,
        "<twice>",
    )
    .expect("the file parses");

    let report = registry.validate();
    assert_eq!(report.count(Severity::Error), 1, "{report:?}");
    assert_eq!(report.diagnostics()[0].code.as_str(), codes::DUPLICATE_ID);
    assert_eq!(
        registry.find("twice").map(|e| e.name.as_str()),
        Some("First")
    );
}
