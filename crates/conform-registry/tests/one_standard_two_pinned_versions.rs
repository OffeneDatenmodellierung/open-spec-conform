//! One specification, two pinned versions: the format this was built for.
//!
//! `ossie` is catalogued once and carries two `[[spec.pin]]` blocks: the
//! released version (0.1.1) and the in-development draft (0.2.0.dev0) that
//! implementers are building against. The duplicate-id rule (`REG005`) is not
//! involved — there is one id, and one entry.
//!
//! # What this test pins
//!
//! The two versions agree on nothing that names a *revision* (version string,
//! pin, vendored path, digest) and sit under one entry whose identity fields
//! (name, repository, homepage, steward, licence) are shared. If the registry
//! ever loses that shape, these tests catch it.
//!
//! # Default version selection
//!
//! `--spec ossie` must resolve to the released version (0.1.1), not the draft.
//! A draft contains `dev`, `alpha`, `beta`, `rc`, `pre` or `SNAPSHOT` in its
//! version string.
//!
//! # The duplicate-version rule
//!
//! Within one entry, two `[[spec.pin]]` blocks with the same version string
//! are refused under `REG011`, exactly as two entries sharing an id are refused
//! under `REG005`.

mod support;

use conform_core::Severity;
use conform_registry::{Registry, codes};

/// The single `ossie` entry, from the real registry.
fn ossie() -> conform_registry::SpecEntry {
    let registry = support::real_registry();
    registry
        .find("ossie")
        .expect("the registry records `ossie`")
        .clone()
}

#[test]
fn one_entry_carries_both_versions() {
    let entry = ossie();
    assert_eq!(
        entry.versions.len(),
        2,
        "the `ossie` entry should have exactly two pinned versions"
    );
}

#[test]
fn the_two_versions_differ_on_everything_that_names_a_revision() {
    let entry = ossie();
    let (a, b) = (&entry.versions[0], &entry.versions[1]);

    assert_ne!(a.version, b.version);
    assert_ne!(a.pinned_ref, b.pinned_ref);
    assert_ne!(a.vendored_path, b.vendored_path);
    assert_ne!(
        a.sha256, b.sha256,
        "two revisions of a document that hash alike are one revision recorded twice"
    );
}

#[test]
fn the_default_version_is_the_non_draft_one() {
    let entry = ossie();
    let default = entry.default_version();
    assert_eq!(
        default.version.as_deref(),
        Some("0.1.1"),
        "the default should be the released (non-draft) version"
    );
    assert!(
        !default.is_draft(),
        "the default version must not be a draft"
    );
}

#[test]
fn the_draft_is_detected_as_a_draft() {
    let entry = ossie();
    let draft = entry
        .find_version("0.2.0.dev0")
        .expect("draft version exists");
    assert!(
        draft.is_draft(),
        "`0.2.0.dev0` should be detected as a draft"
    );
}

#[test]
fn the_in_development_version_is_polled_more_often_than_the_released_one() {
    let entry = ossie();
    let released = entry
        .find_version("0.1.1")
        .expect("released version exists");
    let development = entry
        .find_version("0.2.0.dev0")
        .expect("development version exists");

    let cadence = |v: &conform_registry::PinnedVersion| {
        v.poll
            .as_ref()
            .and_then(|poll| poll.cadence.clone())
            .unwrap_or_else(|| panic!("version {:?} records no poll cadence", v.version))
    };

    assert_eq!(cadence(development), "weekly");
    assert_eq!(cadence(released), "monthly");
}

#[test]
fn the_duplicate_version_rule_bites() {
    let text = support::real_registry_text();
    let sabotaged = text.replacen(
        r#"version       = "0.2.0.dev0""#,
        r#"version       = "0.1.1""#,
        1,
    );
    assert_ne!(
        sabotaged, text,
        "the development version is no longer spelled `0.2.0.dev0`; update this control"
    );

    let registry = Registry::load_str(&sabotaged, "<specs.toml with a repeated version>")
        .expect("the sabotaged registry parses")
        .with_root(support::workspace_root());
    let report = registry.validate();

    assert!(
        report
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_VERSION),
        "two versions sharing a version string went unremarked: {report:?}"
    );

    // And the real registry has no such error.
    assert_eq!(
        support::real_registry().validate().count(Severity::Error),
        0
    );
}

#[test]
fn the_duplicate_id_rule_still_bites() {
    let text = support::real_registry_text();
    // Add a second entry with the same id.
    let sabotaged = format!(
        "{text}\n\n[[spec]]\nid = \"ossie\"\nname = \"Duplicate\"\n\n[[spec.pin]]\n\
         vendored_path = \"schemas/fake.json\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
         fetched_at = \"2026-01-01\"\n"
    );

    let registry = Registry::load_str(&sabotaged, "<specs.toml with a repeated id>")
        .expect("the sabotaged registry parses")
        .with_root(support::workspace_root());
    let report = registry.validate();

    assert!(
        report
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_ID),
        "two entries sharing an id went unremarked: {report:?}"
    );
}
