//! Round-trip fidelity of the DBMV model against the fixture corpus.
//!
//! The property under test is **field preservation**: a document read into
//! [`DBMVDocument`] and written back out is the same document, key for key and
//! value for value. Asserted by exact equality of the two parse trees.
//!
//! It matters more here than in the other model crates. The inner content of a
//! metric view is Databricks', it gains keys on Databricks' schedule, and
//! there is no vendored schema to tell us when it has — so `extra` is the only
//! thing standing between a read-modify-write and silent deletion of a key
//! this model has not heard of. `a_key_this_model_has_never_heard_of_survives`
//! is the test that says so.
//!
//! These fixtures are authored here rather than copied from an adapter corpus:
//! DBMV has no adapter in this workspace and no vendored schema, because
//! Databricks publishes the format as prose. That is a weaker provenance than
//! `conform-model-odcs` and `-odps` have, and it is stated rather than glossed.

use std::fs;
use std::path::{Path, PathBuf};

use conform_model_dbmv::DBMVDocument;
use serde_json::Value;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixtures() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = fs::read_dir(fixture_dir())
        .expect("fixture directory is readable")
        .map(|entry| {
            let path = entry.expect("directory entry is readable").path();
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
        found.len() >= 2,
        "expected the DBMV corpus, found {} file(s)",
        found.len()
    );
    found
}

#[test]
fn every_fixture_round_trips_without_losing_a_single_key() {
    for (name, text) in fixtures() {
        let before: Value = serde_norway::from_str(&text).expect("fixture parses as YAML");

        let model: DBMVDocument = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{name} did not deserialize into the model: {e}"));

        let after = serde_json::to_value(&model)
            .unwrap_or_else(|e| panic!("{name} did not re-serialize: {e}"));

        assert_eq!(
            before, after,
            "{name} changed across a round trip through DBMVDocument"
        );
    }
}

#[test]
fn the_full_fixture_is_readable_as_typed_values() {
    let text = fs::read_to_string(fixture_dir().join("full.dbmv.yaml")).unwrap();
    let doc: DBMVDocument = serde_norway::from_str(&text).unwrap();

    assert_eq!(doc.api_version, "v1.0.0");
    assert_eq!(doc.kind, "MetricViews");
    assert_eq!(doc.system, "retail-warehouse");

    let view = doc.get_metric_view("orders_metrics").expect("metric view");
    assert_eq!(view.source, "catalog.retail.orders");
    assert_eq!(view.filter.as_deref(), Some("status = 'completed'"));
    assert_eq!(view.version, "1.1");

    assert_eq!(view.dimensions.len(), 2);
    assert_eq!(view.dimensions[0].expr, "order_date");
    assert_eq!(
        view.dimensions[1].display_name.as_deref(),
        Some("Customer Nation")
    );

    assert_eq!(view.measures.len(), 2);
    assert_eq!(
        view.measures[0].format.as_ref().unwrap().format_type,
        "currency"
    );
    let window = &view.measures[1].window[0];
    assert_eq!(window.order, "order_date");
    assert_eq!(window.range.as_deref(), Some("cumulative"));
    assert_eq!(window.semiadditive.as_deref(), Some("last"));

    // Nested joins, which is what makes this a snowflake schema.
    assert_eq!(view.joins.len(), 1);
    assert_eq!(view.joins[0].name, "customers");
    assert_eq!(view.joins[0].joins[0].source, "catalog.retail.nations");
    assert_eq!(view.joins[0].joins[0].using, vec!["nation_id"]);

    let mat = view.materialization.as_ref().expect("materialization");
    assert_eq!(mat.schedule, "every 6 hours");
    assert_eq!(mat.materialized_views.len(), 2);
    assert_eq!(mat.materialized_views[1].view_type, "aggregated");

    // The lineage question the document is read to answer.
    assert_eq!(
        doc.source_tables(),
        vec![
            "catalog.retail.orders",
            "catalog.retail.customers",
            "catalog.retail.nations",
        ]
    );
}

#[test]
fn a_key_this_model_has_never_heard_of_survives() {
    // There is no vendored DBMV schema, so the day Databricks adds a key is a
    // day nothing in this repository will notice. `extra` is what makes that
    // survivable rather than destructive.
    let text = fs::read_to_string(fixture_dir().join("full.dbmv.yaml")).unwrap();
    let text = text.replace(
        "        format:\n          type: currency",
        "        format:\n          type: currency\n          decimal_places: 2\n        aggregation_hint: additive",
    );

    let doc: DBMVDocument = serde_norway::from_str(&text).unwrap();
    let measure = &doc.metric_views[0].measures[0];
    assert_eq!(
        measure.format.as_ref().unwrap().extra.get("decimal_places"),
        Some(&serde_json::json!(2))
    );
    assert_eq!(
        measure.extra.get("aggregation_hint"),
        Some(&Value::String("additive".into()))
    );

    let before: Value = serde_norway::from_str(&text).unwrap();
    let after = serde_json::to_value(&doc).unwrap();
    assert_eq!(before, after);
}

#[test]
fn an_explicitly_empty_collection_is_written_back_absent() {
    let json = r#"{"apiVersion":"v1.0.0","kind":"MetricViews","system":"s","metricViews":[]}"#;
    let doc: DBMVDocument = serde_json::from_str(json).unwrap();
    let back = serde_json::to_string(&doc).unwrap();
    assert!(!back.contains("metricViews"), "got {back}");

    // An unknown key, by contrast, is never collapsed -- not even an empty one.
    let json = r#"{"apiVersion":"v1.0.0","kind":"MetricViews","system":"s","x-vendor":[]}"#;
    let doc: DBMVDocument = serde_json::from_str(json).unwrap();
    let back: Value = serde_json::to_value(&doc).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(json).unwrap());
}
