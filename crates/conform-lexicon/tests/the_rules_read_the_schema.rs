//! Three of this crate's rules are derived from the vendored schema rather
//! than transcribed into it. This is what stops that derivation going quiet.
//!
//! A transcribed list is a list that goes stale: upstream adds a root key, or
//! deprecates another field, and a hand-written constant keeps reporting last
//! year's document. So [`crate::rules::SchemaFacts`] reads the permitted root
//! keys, the conventional statuses and the deprecation messages out of the
//! schema at construction, and none of them appears anywhere in this crate's
//! source.
//!
//! That has a failure mode of its own, and it is the quiet kind. Every lookup
//! is tolerant — a schema that no longer has the shape those pointers expect
//! yields an empty set, and the rule simply stops firing. Nothing crashes,
//! nothing gates, and a validator that has stopped checking something looks
//! exactly like a validator that checked and found nothing.
//!
//! These tests read the vendored schema independently of the crate, count what
//! is in it, and assert the rules see the same number. An upstream reshuffle
//! that empties one of those sets fails here instead of passing silently.

mod support;

use std::fs;

use conform_core::Validator;
use conform_lexicon::codes;
use serde_json::Value;

/// The vendored schema, read here rather than through the crate — otherwise
/// both sides of every assertion below would come from the same place.
fn schema() -> Value {
    let path = support::workspace_root().join("schemas/odcl-json-schema-1.2.1.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("the vendored schema is JSON")
}

#[test]
fn the_root_key_rule_sees_every_key_the_schema_names() {
    let schema = schema();
    let named = schema["properties"]
        .as_object()
        .expect("the schema names root properties")
        .len();
    assert_eq!(
        named, 10,
        "the vendored schema names {named} root properties, not the 10 this corpus was written \
         against; review what changed before absorbing it"
    );

    let validator = support::validator();

    // Every key the schema names must pass unremarked...
    let report = validator.validate(&support::fixture("conformant-full.yaml"));
    assert_eq!(
        support::count_code(&report, codes::UNKNOWN_ROOT_KEY),
        0,
        "a document using only keys the schema names was flagged: {:?}",
        support::codes_in(&report)
    );

    // ...and the two that are not named must both be caught. Two, not one:
    // a rule that fired on the first offender and stopped would look identical
    // on a single-offender fixture.
    let report = validator.validate(&support::fixture("conformant-open-root-key.yaml"));
    assert_eq!(
        support::count_code(&report, codes::UNKNOWN_ROOT_KEY),
        2,
        "expected `modles` and `servicelevel` both to be caught: {:?}",
        support::codes_in(&report)
    );
}

#[test]
fn the_root_is_genuinely_open_which_is_why_the_rule_exists() {
    let schema = schema();
    assert!(
        schema.get("additionalProperties").is_none()
            && schema.get("unevaluatedProperties").is_none(),
        "the ODCL root now closes its properties, so the schema catches unknown keys itself and \
         `UNKNOWN_ROOT_KEY` has become redundant — remove the rule rather than leaving two \
         findings for one mistake"
    );

    // And the consequence, end to end: the published schema accepts the
    // misspelling. If this ever fails, the rule above has stopped being the
    // only thing standing between a typo and silence.
    let validator = support::validator();
    let report = validator.validate(&support::fixture("conformant-open-root-key.yaml"));
    assert!(
        support::errors(&report).is_empty(),
        "the schema now rejects the misspelled-root fixture: {:?}",
        support::errors(&report)
    );
}

#[test]
fn the_deprecation_rule_sees_every_annotation_the_schema_carries() {
    // Count them in the schema, by walking it — not by trusting the two
    // pointers the crate uses, which is the thing being checked.
    fn count(node: &Value) -> usize {
        match node {
            Value::Object(members) => {
                usize::from(members.contains_key("deprecationMessage"))
                    + members.values().map(count).sum::<usize>()
            }
            Value::Array(items) => items.iter().map(count).sum(),
            _ => 0,
        }
    }
    let carried = count(&schema());
    assert_eq!(
        carried, 5,
        "the vendored schema carries {carried} `deprecationMessage` annotations, not the 5 this \
         test was written against"
    );

    let validator = support::validator();
    let report = validator.validate(&support::fixture("conformant-deprecated-keywords.yaml"));
    assert_eq!(
        support::count_code(&report, codes::DEPRECATED_KEYWORD),
        carried,
        "the fixture uses all {carried} deprecated keys and only some were reported: {:?}",
        support::codes_in(&report)
    );

    // Upstream's own wording, quoted rather than paraphrased.
    let messages: Vec<&str> = report
        .iter()
        .filter(|d| d.code.as_str() == codes::DEPRECATED_KEYWORD)
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Use the primaryKey field instead.")),
        "the schema's own deprecation message was not carried through: {messages:?}"
    );

    // The control: a document using none of them says nothing about any.
    let clean = validator.validate(&support::fixture("conformant-full.yaml"));
    assert_eq!(support::count_code(&clean, codes::DEPRECATED_KEYWORD), 0);
}

#[test]
fn the_status_rule_sees_the_examples_the_schema_publishes() {
    let schema = schema();
    let published: Vec<&str> = schema["properties"]["info"]["properties"]["status"]["examples"]
        .as_array()
        .expect("the schema publishes status examples")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        published,
        [
            "proposed",
            "in development",
            "active",
            "deprecated",
            "retired"
        ],
        "the conventional status set has changed upstream"
    );
    assert!(
        schema["properties"]["info"]["properties"]["status"]
            .get("enum")
            .is_none(),
        "`info.status` is now an enum, so the schema enforces it and this rule is redundant"
    );

    let validator = support::validator();

    // A published value must not be flagged — `active`, used by most of the
    // corpus, is the control.
    let clean = validator.validate(&support::fixture("conformant-full.yaml"));
    assert_eq!(
        support::count_code(&clean, codes::UNCONVENTIONAL_STATUS),
        0,
        "`status: active` was flagged as unconventional"
    );

    let report = validator.validate(&support::fixture("conformant-unconventional-status.yaml"));
    assert_eq!(
        support::count_code(&report, codes::UNCONVENTIONAL_STATUS),
        1,
        "`status: mothballed` was not flagged: {:?}",
        support::codes_in(&report)
    );
}

/// The whole reason those rules exist: the published schema accepts every one
/// of the documents they complain about.
///
/// If any of these started failing the schema, the corresponding rule would be
/// a second finding for a mistake already reported — worth deleting, not worth
/// keeping.
#[test]
fn every_hygiene_fixture_conforms_to_the_published_schema() {
    let validator = support::validator();
    for name in [
        "conformant-open-root-key.yaml",
        "conformant-deprecated-keywords.yaml",
        "conformant-unconventional-status.yaml",
        "conformant-untyped-field.yaml",
        "conformant-dangling-references.yaml",
        "conformant-server-dispatch-is-dead.yaml",
        "conformant-older-version.yaml",
    ] {
        let report = validator.validate(&support::fixture(name));
        assert!(
            support::errors(&report).is_empty(),
            "{name} is meant to be a document the schema ACCEPTS and something raised an error \
             on it: {:?}",
            support::errors(&report)
        );
        assert!(
            report.len() > 1,
            "{name} produced nothing but the provenance note, so whatever it was written to \
             demonstrate is not being demonstrated"
        );
    }
}
