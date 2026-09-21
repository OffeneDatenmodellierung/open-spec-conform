//! The schema's provenance is checked before it is used, and that is a test
//! rather than a claim in a doc comment.
//!
//! This matters more here than the structure of the test suggests. The
//! pre-existing `validate_odcl_internal` reaches its schema with
//! `include_str!("../../../../schemas/odcl-json-schema-1.2.1.json")`: no
//! digest, no record of which upstream revision those bytes are, and nothing
//! that would notice if somebody edited them. The version in the path is the
//! only provenance there is, and a path is not evidence. This crate resolves
//! the same schema through `conform-registry` instead, and refuses to run
//! against bytes that no longer hash to what `specs.toml` records.
//!
//! Saying "we resolve the schema through the registry" is cheap. What it has
//! to mean is that a schema whose bytes no longer match the digest
//! `specs.toml` records is *refused*, not used to issue confident verdicts.
//! Each check here has its own control, because a refusal that fires on
//! everything and a refusal that fires on nothing both look like a passing
//! test from one direction.

mod support;

use std::fs;
use std::path::PathBuf;

use conform_core::{Severity, Validator};
use conform_lexicon::{LexiconValidator, codes};

/// A scratch directory holding a registry and whatever artefacts a test wants
/// it to describe. Removed on drop, so a failing test does not leave debris
/// behind for the next one to trip over.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "conform-lexicon-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("schemas")).expect("cannot create the scratch directory");
        Self { root }
    }

    /// Copy the real vendored schema in, optionally corrupting it.
    fn with_schema(&self, edit: impl FnOnce(String) -> String) -> &Self {
        let source = support::workspace_root().join("schemas/odcl-json-schema-1.2.1.json");
        let text = fs::read_to_string(&source)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", source.display()));
        fs::write(
            self.root.join("schemas/odcl-json-schema-1.2.1.json"),
            edit(text),
        )
        .expect("cannot write the scratch schema");
        self
    }

    fn with_registry(&self, toml: &str) -> PathBuf {
        let path = self.root.join("specs.toml");
        fs::write(&path, toml).expect("cannot write the scratch registry");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A registry entry for the real vendored schema, with whichever digest the
/// caller wants recorded against it.
fn registry_with_digest(digest: &str) -> String {
    format!(
        r#"schema_version = 1

[[spec]]
id            = "odcl"
name          = "Data Contract Specification"
version       = "1.2.1"
repository    = "https://github.com/datacontract/datacontract-specification"
pinned_ref    = "1.2.1"
vendored_path = "schemas/odcl-json-schema-1.2.1.json"
sha256        = "{digest}"
fetched_at    = "2026-09-20"
notes         = "A scratch registry, written by tests/provenance_is_enforced.rs."
"#
    )
}

/// The digest `specs.toml` records for the real artefact.
fn real_digest() -> String {
    let bytes = fs::read(support::workspace_root().join("schemas/odcl-json-schema-1.2.1.json"))
        .expect("cannot read the vendored schema");
    conform_registry::sha256_hex(&bytes)
}

fn codes_in(error: &conform_lexicon::SchemaError) -> Vec<String> {
    error
        .report()
        .iter()
        .map(|d| d.code.as_str().to_owned())
        .collect()
}

/// The control: the same scratch setup, honest, builds a validator.
///
/// Without this, every assertion below would be satisfied by a constructor
/// that refused unconditionally.
#[test]
fn an_honest_registry_builds_a_validator() {
    let scratch = Scratch::new("honest");
    scratch.with_schema(|text| text);
    let registry = scratch.with_registry(&registry_with_digest(&real_digest()));

    let validator = LexiconValidator::from_registry_path(&registry)
        .expect("a registry whose digest matches its artefact should build a validator");
    assert!(
        validator.provenance().contains("verified"),
        "the provenance note does not say the bytes were checked: {}",
        validator.provenance()
    );
}

#[test]
fn a_schema_that_no_longer_matches_its_digest_is_refused() {
    let scratch = Scratch::new("drifted");
    // One byte of difference is enough, and it is a difference that changes
    // nothing about what the schema *means* — which is the point. Provenance
    // is about bytes, not about semantics.
    scratch.with_schema(|text| text.replacen('{', "{ ", 1));
    let registry = scratch.with_registry(&registry_with_digest(&real_digest()));

    let error = LexiconValidator::from_registry_path(&registry)
        .expect_err("a drifted schema must not produce a working validator");
    let raised = codes_in(&error);

    assert!(
        raised.iter().any(|c| c == codes::SCHEMA_PROVENANCE_FAILED),
        "the refusal did not carry {}: {raised:?}",
        codes::SCHEMA_PROVENANCE_FAILED
    );
    // And the registry's own diagnostic is passed through verbatim rather than
    // paraphrased, so the reader gets its code and its explanation too.
    assert!(
        raised
            .iter()
            .any(|c| c == conform_registry::codes::SHA256_MISMATCH),
        "the registry's own {} diagnostic was swallowed: {raised:?}",
        conform_registry::codes::SHA256_MISMATCH
    );
    assert!(
        error.report().at_or_above(Severity::Error).count() >= 2,
        "a refusal should explain itself in more than one sentence"
    );
}

#[test]
fn a_missing_schema_is_refused() {
    let scratch = Scratch::new("missing");
    // Deliberately no `with_schema`: the registry describes an artefact that
    // is not there.
    let registry = scratch.with_registry(&registry_with_digest(&real_digest()));

    let error = LexiconValidator::from_registry_path(&registry)
        .expect_err("a registry describing an absent artefact must not build a validator");
    let raised = codes_in(&error);
    assert!(
        raised
            .iter()
            .any(|c| c == conform_registry::codes::ARTEFACT_MISSING),
        "expected the registry's artefact-missing code: {raised:?}"
    );
}

#[test]
fn a_registry_without_this_standard_is_refused() {
    let scratch = Scratch::new("absent-entry");
    scratch.with_schema(|text| text);
    let registry = scratch.with_registry(
        r#"schema_version = 1

[[spec]]
id            = "something-else"
name          = "Not the standard this crate speaks for"
version       = "1.0"
homepage      = "https://example.org/"
repository    = "https://github.com/example/example"
steward       = "Example"
licence       = "MIT"
pinned_ref    = "v1.0"
vendored_path = "schemas/odcl-json-schema-1.2.1.json"
sha256        = "0000000000000000000000000000000000000000000000000000000000000000"
fetched_at    = "2026-09-20"
notes         = "A scratch registry, written by tests/provenance_is_enforced.rs."
"#,
    );

    let error = LexiconValidator::from_registry_path(&registry)
        .expect_err("a registry with no entry for this standard must not build a validator");
    assert_eq!(codes_in(&error), vec![codes::NOT_IN_REGISTRY.to_owned()]);
}

#[test]
fn the_real_registry_is_the_one_this_crate_uses() {
    // Not a scratch registry: the actual `specs.toml` at the repository root,
    // loaded the way a consumer loads it. If this crate could only be built
    // from a fixture, the constructor would be a test artefact rather than an
    // API.
    let validator = support::validator();
    let spec = validator.spec();

    assert_eq!(spec.id, conform_lexicon::SPEC_ID);
    assert_eq!(
        spec.version.as_deref(),
        Some("1.2.1"),
        "the real registry no longer pins the version this crate's tests were written against; \
         that is a change to review, not to absorb"
    );
    assert!(
        validator.provenance().contains("verified"),
        "the provenance note does not say the bytes were checked: {}",
        validator.provenance()
    );
}
