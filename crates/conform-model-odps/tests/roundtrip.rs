//! Round-trip fidelity of the ODPS model against the real fixture corpus.
//!
//! The property under test is **field preservation**: a document read into
//! [`ODPSDataProduct`] and written back out is the same document, key for key
//! and value for value, including everything the model does not name. It is
//! asserted by exact equality of the two parse trees rather than by
//! spot-checking fields, because a spot check is exactly the test that passes
//! while a vendor extension quietly disappears.
//!
//! The fixtures are copied verbatim from `crates/conform-odps/tests/fixtures/`,
//! the corpus the ODPS adapter is pinned against; `tests/fixtures/PROVENANCE.md`
//! records which commit they were taken at.
//!
//! They are *copies* rather than a path into the sibling crate, and that is
//! deliberate: this crate is published to crates.io, and a test reading
//! `../conform-odps/tests/fixtures` would pass in the workspace and fail in the
//! tarball, because the sibling is not in it. Every path this crate touches
//! stays inside this crate.

use std::fs;
use std::path::{Path, PathBuf};

use conform_model_odps::ODPSDataProduct;
use serde_json::Value;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixtures() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = fs::read_dir(fixture_dir())
        .expect("fixture directory is readable")
        .map(|entry| entry.expect("directory entry is readable").path())
        // The directory also holds PROVENANCE.md, which is documentation
        // rather than a document to round-trip.
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yaml" | "yml" | "json")
            )
        })
        .map(|path| {
            let name = path
                .file_name()
                .expect("fixture has a file name")
                .to_string_lossy()
                .into_owned();
            let text = fs::read_to_string(&path).expect("fixture is readable");
            (name, text)
        })
        .collect();
    found.sort();
    assert!(
        found.len() >= 4,
        "expected the conformant ODPS corpus, found {} file(s)",
        found.len()
    );
    found
}

#[test]
fn every_conformant_fixture_round_trips_without_losing_a_single_key() {
    for (name, text) in fixtures() {
        let before: Value = serde_norway::from_str(&text).expect("fixture parses as YAML");

        let model: ODPSDataProduct = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{name} did not deserialize into the model: {e}"));

        let after = serde_json::to_value(&model)
            .unwrap_or_else(|e| panic!("{name} did not re-serialize: {e}"));

        assert_eq!(
            before, after,
            "{name} changed across a round trip through ODPSDataProduct"
        );
    }
}

#[test]
fn the_full_fixture_is_readable_as_typed_values() {
    let text = fs::read_to_string(fixture_dir().join("conformant-full.yaml")).unwrap();
    let product: ODPSDataProduct = serde_norway::from_str(&text).unwrap();

    assert_eq!(product.api_version, "v1.0.0");
    assert_eq!(product.kind, "DataProduct");
    assert_eq!(product.status, "active");
    assert_eq!(product.name.as_deref(), Some("Order Analytics"));
    assert_eq!(product.version.as_deref(), Some("1.4.0"));
    assert_eq!(product.domain.as_deref(), Some("sales"));
    assert_eq!(product.tenant.as_deref(), Some("RetailCorp"));
    assert_eq!(product.tags, vec!["sales", "gold"]);

    let raw = product.input_port("raw-orders").expect("input port");
    assert_eq!(raw.version, "2.3.0");
    assert_eq!(raw.contract_id, "4b1f0f0e-2bb8-4a9d-9c17-4a4cbe3f2f2a");

    let gold = product.output_port("orders_gold").expect("output port");
    assert_eq!(gold.port_type.as_deref(), Some("tables"));
    assert_eq!(
        gold.contract_id.as_deref(),
        Some("7a2e9c41-11f0-4c2a-9a6f-2f1d6e5b3c88")
    );
    assert_eq!(gold.input_contracts.len(), 1);
    assert_eq!(gold.input_contracts[0].version, "2.3.0");

    assert_eq!(product.management_ports.len(), 1);
    assert_eq!(product.management_ports[0].content, "discoverability");
    assert_eq!(
        product.management_ports[0].port_type.as_deref(),
        Some("rest")
    );

    assert_eq!(product.support.len(), 1);
    assert_eq!(product.support[0].channel, "#retail-data");

    let team = product.team.as_ref().expect("team");
    assert_eq!(team.name.as_deref(), Some("Retail Data Platform"));
    assert_eq!(team.members.len(), 1);
    assert_eq!(team.members[0].username, "ceastwood");

    assert_eq!(
        product.description.as_ref().unwrap().purpose.as_deref(),
        Some("Publish curated order facts for analytics consumers.")
    );

    // The question an ODPS document is read to answer.
    assert_eq!(
        product.contract_ids(),
        vec![
            "4b1f0f0e-2bb8-4a9d-9c17-4a4cbe3f2f2a",
            "7a2e9c41-11f0-4c2a-9a6f-2f1d6e5b3c88",
            "4b1f0f0e-2bb8-4a9d-9c17-4a4cbe3f2f2a",
        ]
    );
}

#[test]
fn an_explicitly_empty_collection_is_written_back_absent() {
    // The one normalisation this model performs, stated so it cannot be
    // mistaken for the unknown-key guarantee. ODPS gives `tags: []` and an
    // absent `tags` the same meaning, so collapsing them loses nothing a
    // reader could act on -- but it *is* a change, and it is tested rather
    // than left for someone to discover.
    let json =
        r#"{"apiVersion":"v1.0.0","kind":"DataProduct","id":"x","status":"active","tags":[]}"#;
    let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
    assert!(product.tags.is_empty());

    let back = serde_json::to_string(&product).unwrap();
    assert!(
        !back.contains("tags"),
        "an empty collection should be omitted, got {back}"
    );

    // An unknown key, by contrast, is never collapsed -- not even an empty one.
    let json =
        r#"{"apiVersion":"v1.0.0","kind":"DataProduct","id":"x","status":"active","x-vendor":[]}"#;
    let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
    let back: Value = serde_json::to_value(&product).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(json).unwrap());
}
