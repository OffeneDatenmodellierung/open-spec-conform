//! Round-trip fidelity of the ODCS model against the real fixture corpus.
//!
//! The property under test is **field preservation**: a document read into
//! [`ODCSContract`] and written back out is the same document. Not "the fields
//! we modelled survived" — the *whole* document, key for key and value for
//! value, including everything the model does not name.
//!
//! That is asserted by exact equality of the two parse trees rather than by
//! spot-checking a handful of fields, because a spot check is exactly the test
//! that would pass while a vendor extension quietly disappeared.
//!
//! The fixtures are copied verbatim from `crates/conform-odcs/tests/fixtures/`,
//! the corpus the ODCS adapter is pinned against; `tests/fixtures/PROVENANCE.md`
//! records which commit they were taken at.
//!
//! They are *copies* rather than a path into the sibling crate, and that is
//! deliberate: this crate is published to crates.io, and a test reading
//! `../conform-odcs/tests/fixtures` would pass in the workspace and fail in the
//! tarball, because the sibling is not in it. Every path this crate touches
//! stays inside this crate.

use std::fs;
use std::path::{Path, PathBuf};

use conform_model_odcs::ODCSContract;
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
        "expected the conformant ODCS corpus, found {} file(s)",
        found.len()
    );
    found
}

/// Parse a fixture as an untyped tree, which is the ground truth to compare against.
///
/// Both YAML and JSON go through `serde_norway`: JSON is a subset of YAML, and
/// using one parser for both means any difference between the two sides of the
/// comparison is the model's doing rather than two parsers disagreeing.
fn as_tree(text: &str) -> Value {
    serde_norway::from_str(text).expect("fixture parses as YAML")
}

#[test]
fn every_conformant_fixture_round_trips_without_losing_a_single_key() {
    for (name, text) in fixtures() {
        let before = as_tree(&text);

        let model: ODCSContract = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{name} did not deserialize into the model: {e}"));

        let after = serde_json::to_value(&model)
            .unwrap_or_else(|e| panic!("{name} did not re-serialize: {e}"));

        assert_eq!(
            before, after,
            "{name} changed across a round trip through ODCSContract"
        );
    }
}

#[test]
fn the_full_fixture_is_readable_as_typed_values() {
    // The point of the crate: a third party can reach into the document
    // without going near a validator. Each of these would have been
    // unreachable through `Document { id, text }`.
    let text = fs::read_to_string(fixture_dir().join("conformant-full.yaml")).unwrap();
    let contract: ODCSContract = serde_norway::from_str(&text).unwrap();

    assert_eq!(contract.api_version, "v3.1.0");
    assert_eq!(contract.kind, "DataContract");
    assert_eq!(contract.status, "active");
    assert_eq!(contract.name.as_deref(), Some("Orders"));
    assert_eq!(contract.domain.as_deref(), Some("sales"));
    assert_eq!(contract.tags, vec!["sales", "gold"]);

    // servers[0].host — the field named in the audit as unreachable.
    let server = &contract.servers[0];
    assert_eq!(server.server, "orders-prod");
    assert_eq!(server.server_type, "postgresql");
    assert_eq!(server.host.as_deref(), Some("db.example.com"));
    assert_eq!(server.port, Some(5432));
    assert_eq!(server.database.as_deref(), Some("retail"));

    // The schema hierarchy.
    assert_eq!(contract.schema_count(), 1);
    let orders = contract
        .get_schema("orders")
        .expect("schema object 'orders'");
    assert_eq!(orders.logical_type.as_deref(), Some("object"));
    assert_eq!(orders.physical_name.as_deref(), Some("orders_v2"));
    assert_eq!(orders.property_count(), 3);

    let order_id = orders
        .get_property("order_id")
        .expect("property 'order_id'");
    assert!(order_id.primary_key);
    assert_eq!(order_id.primary_key_position, Some(1));
    assert!(order_id.required);
    assert!(order_id.unique);
    assert_eq!(order_id.physical_type.as_deref(), Some("uuid"));

    // Structured description.
    assert_eq!(
        contract.description_string().as_deref(),
        Some("Serve completed retail orders to downstream analytics.")
    );

    // Support is an array in ODCS v3.1.0, and one channel is still an array.
    assert_eq!(contract.support.len(), 1);
    assert_eq!(contract.support[0].channel, "#retail-data");
    assert_eq!(contract.support[0].tool.as_deref(), Some("slack"));

    // Team, under the v3 object spelling.
    let team = contract.team.as_ref().expect("team");
    assert_eq!(team.members().len(), 1);
    assert_eq!(team.members()[0].username, "ceastwood");
    assert_eq!(team.members()[0].date_in.as_deref(), Some("2025-04-01"));

    // `slaProperties`, which the upstream model spelled `serviceLevels`.
    assert_eq!(contract.sla_properties.len(), 1);
    assert_eq!(contract.sla_properties[0].property, "latency");
    assert_eq!(contract.sla_properties[0].value, serde_json::json!(4));
    assert_eq!(contract.sla_properties[0].unit.as_deref(), Some("h"));
}

#[test]
fn keys_the_model_does_not_name_land_in_extra_rather_than_nowhere() {
    // `path` is legal on a `local` server in ODCS but is not one of the keys
    // this model names, so it must be reachable through `extra`.
    let text = fs::read_to_string(fixture_dir().join("conformant-older-api-version.json")).unwrap();
    let contract: ODCSContract = serde_norway::from_str(&text).unwrap();

    assert_eq!(
        contract.servers[0].extra.get("path"),
        Some(&Value::String("./data/*.parquet".into())),
        "an unmodelled server key must survive in `extra`"
    );
}

#[test]
fn an_unconventional_status_is_read_rather_than_rejected() {
    // ODCS publishes `status` values as `examples`, not as an `enum`. A model
    // that closed the set would refuse a conformant document.
    let text =
        fs::read_to_string(fixture_dir().join("conformant-unconventional-status.yaml")).unwrap();
    let contract: ODCSContract = serde_norway::from_str(&text).unwrap();
    assert_eq!(contract.status, "mothballed");
}

#[test]
fn an_explicitly_empty_collection_is_written_back_absent() {
    // The one normalisation this model performs, stated so it cannot be
    // mistaken for the unknown-key guarantee. ODCS gives `tags: []` and an
    // absent `tags` the same meaning, so collapsing them loses nothing a
    // reader could act on -- but it *is* a change, and it is tested rather
    // than left for someone to discover.
    let json = r#"{"version":"1.0.0","apiVersion":"v3.1.0","kind":"DataContract",
                   "id":"x","status":"active","tags":[]}"#;
    let contract: ODCSContract = serde_json::from_str(json).unwrap();
    assert!(contract.tags.is_empty());

    let back = serde_json::to_string(&contract).unwrap();
    assert!(
        !back.contains("tags"),
        "an empty collection should be omitted, got {back}"
    );

    // An unknown key, by contrast, is never collapsed -- not even an empty one.
    let json = r#"{"version":"1.0.0","apiVersion":"v3.1.0","kind":"DataContract",
                   "id":"x","status":"active","x-vendor":[]}"#;
    let contract: ODCSContract = serde_json::from_str(json).unwrap();
    let back: Value = serde_json::to_value(&contract).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(json).unwrap());
}
