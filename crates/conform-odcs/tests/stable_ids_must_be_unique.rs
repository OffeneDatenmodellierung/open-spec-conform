//! `$defs/StableId` says identifiers must be unique. Nothing enforced it.
//!
//! The vendored schema's own description of that definition reads *"Stable
//! technical identifier for references. Must be unique within its containing
//! array."* — and the definition it is attached to constrains a `pattern` and
//! nothing else. `/properties/schema` and `/properties/servers` carry no
//! `uniqueItems`, and `uniqueItems` could not say this anyway: it compares
//! whole members, so two schema objects sharing an `id` and differing in a
//! name are already unique by that keyword's reckoning.
//!
//! So the sentence was unenforceable by construction, and a contract with two
//! `schema[].id` of `orders` passed clean. This file is what makes that stop
//! being true, and — just as importantly — what shows the new rule does *not*
//! fire on the document next to it that is correct.
//!
//! # Why these two documents are not in `tests/fixtures/`
//!
//! That directory is the **oracle corpus**, and `tests/oracle_agreement.rs`
//! holds it to a two-way completeness property: every fixture there has a
//! recorded verdict from the validator this crate replaces, and every record
//! has a fixture. Its purpose is pinning verdict *equivalence* across the
//! migration. A document exercising a rule the old validator has never heard
//! of adds nothing to that evidence — it would only grow a corpus that is
//! nearly closed. So the two documents this file is about live beside it, in
//! `tests/stable-ids/`, and the oracle corpus stays exactly what it was.

mod support;

use std::fs;
use std::path::PathBuf;

use conform_core::{GatePolicy, Severity, Validator};
use conform_odcs::{Document, codes};

const DUPLICATES: &str = "duplicate-stable-ids.yaml";
const DISTINCT: &str = "distinct-stable-ids.yaml";

/// One of this test's own documents, loaded by name.
fn document(name: &str) -> Document {
    Document::new(name, text(name))
}

/// The raw bytes of one of them, for the assertions that are about what the
/// fixture still says rather than about what the validator found.
fn text(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/stable-ids")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every duplicate the fixture carries, as `(pointer of the repeat, the `id`,
/// pointer of the first occurrence it names)`.
///
/// Spelled out rather than counted, because a count would pass if the rule
/// found four duplicates in the wrong places.
const EXPECTED: &[(&str, &str, &str)] = &[
    ("/servers/1/id", "primary", "/servers/0"),
    (
        "/schema/0/properties/1/id",
        "order_id",
        "/schema/0/properties/0",
    ),
    (
        "/schema/0/properties/2/items/properties/1/id",
        "sku",
        "/schema/0/properties/2/items/properties/0",
    ),
    ("/schema/1/id", "orders", "/schema/0"),
];

#[test]
fn every_repeated_identifier_is_reported_at_the_repeat() {
    let validator = support::validator();
    let report = validator.validate(&document(DUPLICATES));

    let mut found: Vec<(String, String)> = report
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_STABLE_ID)
        .map(|diagnostic| {
            (
                diagnostic
                    .location
                    .pointer
                    .as_ref()
                    .expect("a duplicate names where it is")
                    .as_str()
                    .to_owned(),
                diagnostic.message.clone(),
            )
        })
        .collect();
    found.sort();

    let mut expected: Vec<(String, String)> = EXPECTED
        .iter()
        .map(|(pointer, id, first)| {
            (
                (*pointer).to_owned(),
                format!("`id` is `{id}`, which `{first}` already carries"),
            )
        })
        .collect();
    expected.sort();

    assert_eq!(
        found, expected,
        "the duplicates reported are not the duplicates the fixture carries"
    );
}

/// The location is the *second* occurrence. Pointing at the first would send
/// a reader to a member that may be entirely correct.
#[test]
fn the_finding_lands_on_the_repeat_and_not_on_the_first_occurrence() {
    let validator = support::validator();
    let report = validator.validate(&document(DUPLICATES));

    for diagnostic in report
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_STABLE_ID)
    {
        let pointer = diagnostic
            .location
            .pointer
            .as_ref()
            .expect("a duplicate names where it is")
            .as_str();
        assert!(
            !matches!(pointer, "/servers/0/id" | "/schema/0/id"),
            "{pointer} is the first occurrence, which is not the one that has to change"
        );
    }
}

/// The control. Same shape, distinct identifiers — including the same `id` in
/// two *different* property arrays, which the specification permits.
#[test]
fn distinct_identifiers_raise_nothing() {
    let validator = support::validator();
    let report = validator.validate(&document(DISTINCT));

    let duplicates: Vec<String> = report
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_STABLE_ID)
        .map(ToString::to_string)
        .collect();

    assert!(
        duplicates.is_empty(),
        "a document with no repeated identifier was reported as having some:\n  {}",
        duplicates.join("\n  ")
    );
}

/// "Unique within its containing array" is per array. `/schema/0/properties`
/// and `/schema/1/properties` each hold an `order_id`, and that is legal.
#[test]
fn the_same_identifier_in_two_different_arrays_is_not_a_duplicate() {
    let control = text(DISTINCT);
    assert_eq!(
        control.matches("id: order_id").count(),
        2,
        "the control fixture no longer carries the same `id` in two different property arrays, \
         which is the case this test exists to pin"
    );

    let report = support::validator().validate(&document(DISTINCT));
    assert_eq!(
        report
            .iter()
            .filter(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_STABLE_ID)
            .count(),
        0
    );
}

/// The verdict does not move. This is the property the whole crate is built
/// on: a warning reports, and the default gate does not look at it.
#[test]
fn the_rule_changes_no_verdict() {
    let validator = support::validator();

    for name in [DUPLICATES, DISTINCT] {
        let report = validator.validate(&document(name));
        assert!(
            report.at_or_above(Severity::Error).next().is_none(),
            "{name} produced errors, which would change the verdict the schema gives:\n  {}",
            support::errors(&report).join("\n  ")
        );
        assert!(
            !report.should_gate(GatePolicy::default()),
            "{name} gates under the default policy, and nothing in the hygiene band may"
        );
    }
}
