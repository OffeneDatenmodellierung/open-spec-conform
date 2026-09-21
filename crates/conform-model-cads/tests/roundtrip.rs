//! Round-trip fidelity of the CADS model against the fixture corpus.
//!
//! The property under test is **field preservation**: a document read into
//! [`CADSAsset`] and written back out is the same document, key for key and
//! value for value, including everything the model does not name. It is
//! asserted by exact equality of the two parse trees rather than by
//! spot-checking fields, because a spot check is exactly the test that passes
//! while a vendor extension quietly disappears.
//!
//! # A difference from the ODCS and ODPS model crates
//!
//! Those two copy their fixtures from an adapter corpus that is pinned against
//! a real upstream validator. CADS has no adapter in this workspace, so there
//! is no such corpus to copy. These fixtures are authored here, against
//! `schemas/cads.schema.json` as vendored and pinned by `specs.toml`, and
//! their conformance was checked against that schema with `jsonschema` at the
//! time of writing. That is a weaker provenance than the other two crates
//! have, and it is stated rather than glossed.
//!
//! `nonconformant-unconventional-values.yaml` is deliberately **not**
//! conformant: it carries values outside every set the schema publishes as an
//! `enum`. The vendored schema rejects it, correctly. The model reads it
//! anyway — see `a_document_the_schema_rejects_still_round_trips`.

use std::fs;
use std::path::{Path, PathBuf};

use conform_model_cads::{
    CADSAsset, CADSComplianceStatus, CADSImpactArea, CADSKind, CADSOpenAPIFormat, CADSPricingModel,
    CADSRiskClassification, CADSStatus,
};
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
        found.len() >= 3,
        "expected the CADS corpus, found {} file(s)",
        found.len()
    );
    found
}

#[test]
fn every_fixture_round_trips_without_losing_a_single_key() {
    for (name, text) in fixtures() {
        let before: Value = serde_norway::from_str(&text).expect("fixture parses as YAML");

        let model: CADSAsset = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{name} did not deserialize into the model: {e}"));

        let after = serde_json::to_value(&model)
            .unwrap_or_else(|e| panic!("{name} did not re-serialize: {e}"));

        assert_eq!(
            before, after,
            "{name} changed across a round trip through CADSAsset"
        );
    }
}

#[test]
fn the_full_fixture_is_readable_as_typed_values() {
    let text = fs::read_to_string(fixture_dir().join("conformant-full.yaml")).unwrap();
    let asset: CADSAsset = serde_norway::from_str(&text).unwrap();

    assert_eq!(asset.api_version, "v1.0");
    assert_eq!(asset.kind, CADSKind::AIModel);
    assert_eq!(asset.name, "sentiment-analysis-model");
    assert_eq!(asset.status, CADSStatus::Production);
    assert_eq!(asset.domain.as_deref(), Some("ai-ml"));
    assert_eq!(asset.tags, vec!["ai", "nlp"]);

    let runtime = asset.runtime.as_ref().expect("runtime");
    assert_eq!(runtime.environment.as_deref(), Some("production"));
    assert_eq!(runtime.endpoints.len(), 1);
    assert_eq!(
        runtime.container.as_ref().unwrap().image.as_deref(),
        Some("registry.example.com/sentiment:1.0.0")
    );
    assert_eq!(
        runtime.resources.as_ref().unwrap().gpu.as_deref(),
        Some("1")
    );

    let sla = asset.sla.as_ref().expect("sla");
    assert_eq!(sla.properties.len(), 2);
    assert_eq!(sla.properties[0].element, "latency_p99");
    assert_eq!(sla.properties[0].value, serde_json::json!(250));
    assert_eq!(sla.properties[0].unit, "ms");
    // The schema allows `value` to be a number *or* a string, and both appear.
    assert_eq!(sla.properties[1].value, serde_json::json!("99.9"));

    let pricing = asset.pricing.as_ref().expect("pricing");
    assert_eq!(pricing.model, Some(CADSPricingModel::PerRequest));
    assert_eq!(pricing.currency.as_deref(), Some("EUR"));

    assert_eq!(asset.team.len(), 2);
    assert_eq!(asset.team[0].role, "Model Owner");

    // The questions a governance reader opens a CADS document to ask.
    assert!(asset.is_high_risk());
    let risk = asset.risk.as_ref().expect("risk");
    assert_eq!(risk.classification, Some(CADSRiskClassification::High));
    assert_eq!(
        risk.impact_areas,
        vec![CADSImpactArea::Fairness, CADSImpactArea::Privacy]
    );
    assert_eq!(risk.mitigations.len(), 2);

    let compliance = asset.compliance.as_ref().expect("compliance");
    assert_eq!(compliance.frameworks.len(), 2);
    assert_eq!(
        compliance.frameworks[1].status,
        CADSComplianceStatus::Compliant
    );
    assert_eq!(compliance.controls[0].id, "AI-07");
    assert!(asset.non_compliant_frameworks().is_empty());

    assert_eq!(asset.validation_profiles[0].required_checks.len(), 2);
    assert_eq!(asset.bpmn_models.len(), 1);
    assert_eq!(asset.dmn_models.len(), 1);
    assert_eq!(asset.open_api_specs[0].format, CADSOpenAPIFormat::Openapi31);

    assert_eq!(
        asset.custom_properties.get("costCentre"),
        Some(&Value::String("RD-114".into()))
    );
    // Timestamps are kept as written rather than reformatted through chrono.
    assert_eq!(asset.created_at.as_deref(), Some("2026-01-15T09:00:00Z"));
}

#[test]
fn a_document_the_schema_rejects_still_round_trips() {
    // Every closed set in this document carries a value outside it. A model
    // that refused would leave a caller unable to inspect or repair the very
    // documents most in need of it.
    let text =
        fs::read_to_string(fixture_dir().join("nonconformant-unconventional-values.yaml")).unwrap();
    let asset: CADSAsset = serde_norway::from_str(&text).unwrap();

    assert_eq!(asset.kind, CADSKind::Other("QuantumAnnealer".into()));
    assert_eq!(asset.status, CADSStatus::Other("mothballed".into()));
    assert_eq!(
        asset.pricing.as_ref().unwrap().model,
        Some(CADSPricingModel::Other("per_qubit_second".into()))
    );
    let risk = asset.risk.as_ref().unwrap();
    assert_eq!(
        risk.classification,
        Some(CADSRiskClassification::Other("catastrophic".into()))
    );
    assert_eq!(
        risk.impact_areas,
        vec![CADSImpactArea::Other("existential".into())]
    );

    let before: Value = serde_norway::from_str(&text).unwrap();
    let after = serde_json::to_value(&asset).unwrap();
    assert_eq!(before, after, "an unusual document must still round trip");
}

#[test]
fn an_explicitly_empty_collection_is_written_back_absent() {
    let json = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","name":"m","version":"1.0.0",
                   "status":"draft","tags":[],"team":[]}"#;
    let asset: CADSAsset = serde_json::from_str(json).unwrap();
    let back = serde_json::to_string(&asset).unwrap();
    assert!(!back.contains("tags"), "got {back}");
    assert!(!back.contains("team"), "got {back}");

    // An unknown key, by contrast, is never collapsed -- not even an empty one.
    let json = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","name":"m","version":"1.0.0",
                   "status":"draft","x-vendor":[]}"#;
    let asset: CADSAsset = serde_json::from_str(json).unwrap();
    let back: Value = serde_json::to_value(&asset).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(json).unwrap());
}
