//! The two non-resolving outcomes are different facts, and this is what keeps
//! them different.
//!
//! `conform-core`'s [`Resolution`] exists because "the thing you referenced is
//! missing" and "I never opened that" are not the same statement, and a
//! validator that reports the second as the first raises a false alarm. A gate
//! that raises false alarms gets switched off, after which it protects
//! nothing.
//!
//! This crate has two genuine cross-references to follow — a field's
//! `references`, and a field's `$ref` — and the published schema types both as
//! a bare string and says nothing about where either points. So all three
//! outcomes really occur here, and each one has to be reachable, distinct and
//! correctly severe.
//!
//! Every assertion below is paired with its control, because a resolver that
//! answered `NotInspected` to everything would satisfy half of them.

mod support;

use conform_core::{NotInspectedReason, Resolution, Severity, Validator};
use conform_lexicon::{codes, resolve_field_reference, resolve_ref};
use serde_json::{Value, json};

fn document() -> Value {
    json!({
        "models": {
            "orders": {
                "fields": {
                    "order_id": { "type": "string" },
                    "shipping": { "type": "object", "fields": { "postcode": { "type": "string" } } },
                    "lines": { "type": "array", "items": { "fields": { "sku": { "type": "string" } } } }
                }
            }
        },
        "definitions": {
            "customer_id": { "type": "string" },
            "sales/total": { "type": "number" },
            "weird~name": { "type": "string" }
        }
    })
}

#[test]
fn a_reference_that_is_there_resolves_and_says_where() {
    let document = document();

    let direct = resolve_field_reference(&document, "orders.order_id");
    assert!(direct.is_resolved());
    assert_eq!(
        direct.resolved().map(ToString::to_string),
        Some("/models/orders/fields/order_id".to_owned()),
        "a resolved reference must hand back where it landed, or the caller learns nothing"
    );

    // The `model.nested_field.field` form the schema documents.
    let nested = resolve_field_reference(&document, "orders.shipping.postcode");
    assert_eq!(
        nested.resolved().map(ToString::to_string),
        Some("/models/orders/fields/shipping/fields/postcode".to_owned())
    );
}

#[test]
fn a_reference_that_is_genuinely_absent_says_so_and_only_that() {
    let document = document();

    for absent in [
        "customers.customer_id", // no such model
        "orders.no_such_field",  // model is there, field is not
        "orders.shipping.no_such_nested_field",
    ] {
        let resolution = resolve_field_reference(&document, absent);
        assert!(
            resolution.does_not_exist(),
            "`{absent}` should be absent, and is {resolution:?}"
        );
        // The distinction that matters: absent is not un-inspected.
        assert!(!resolution.is_not_inspected(), "`{absent}`");
        assert!(!resolution.is_resolved(), "`{absent}`");
    }
}

#[test]
fn what_was_never_followed_is_never_reported_as_absent() {
    let document = document();

    // An external `$ref`. Whether it resolves is a network question, and this
    // crate performs no network access.
    let remote = resolve_ref(
        &document,
        "https://example.org/common.yaml#/definitions/currency",
    );
    assert_eq!(
        remote.not_inspected_reason(),
        Some(NotInspectedReason::OutsideDocumentSet)
    );
    assert!(!remote.does_not_exist());

    // A sibling file, with no scheme. Still another document.
    let sibling = resolve_ref(&document, "common.yaml#/definitions/currency");
    assert_eq!(
        sibling.not_inspected_reason(),
        Some(NotInspectedReason::OutsideDocumentSet)
    );

    // A form this crate does not know how to follow. Saying so beats guessing
    // that `#order_id` means `#/definitions/order_id`.
    for unsupported in ["#order_id", "#"] {
        assert_eq!(
            resolve_ref(&document, unsupported).not_inspected_reason(),
            Some(NotInspectedReason::Unsupported),
            "`{unsupported}`"
        );
    }

    // A `references` with no model in it names nothing to look for.
    assert_eq!(
        resolve_field_reference(&document, "order_id").not_inspected_reason(),
        Some(NotInspectedReason::Unsupported)
    );

    // And the case that motivated the rule: a reference through an ARRAY's
    // element fields. The specification documents `model.field` and
    // `model.nested_field.field` and publishes no spelling for this, so the
    // target is very probably there and this crate is not entitled to say it
    // is not.
    let through_an_array = resolve_field_reference(&document, "orders.lines.sku");
    assert_eq!(
        through_an_array.not_inspected_reason(),
        Some(NotInspectedReason::Unsupported)
    );
    assert!(
        !through_an_array.does_not_exist(),
        "reporting an array's element field as missing is exactly the false alarm this type \
         exists to prevent"
    );
}

#[test]
fn a_definition_reference_resolves_both_readings_of_a_slash() {
    let document = document();

    assert!(resolve_ref(&document, "#/definitions/customer_id").is_resolved());

    // `definitions`' propertyNames pattern permits `/` INSIDE a name, and the
    // specification's own advice is to encode the domain into the id with
    // slashes. Read as a JSON Pointer this descends into a nested object and
    // finds nothing; read as one key it resolves. Both are tried.
    let with_a_slash = resolve_ref(&document, "#/definitions/sales/total");
    assert!(
        with_a_slash.is_resolved(),
        "a definition named `sales/total` was reported absent because only the pointer reading \
         was tried: {with_a_slash:?}"
    );
    assert_eq!(
        with_a_slash.resolved().map(ToString::to_string),
        Some("/definitions/sales~1total".to_owned()),
        "the pointer handed back must be escaped, or it reads as two levels of nesting"
    );

    // RFC 6901 escaping, round-tripped: `~0` in the reference is a literal `~`.
    assert!(resolve_ref(&document, "#/definitions/weird~0name").is_resolved());

    // And a definition that really is not there is still absent.
    assert!(resolve_ref(&document, "#/definitions/absent").does_not_exist());
}

/// The three outcomes, as they reach a reader of a report.
///
/// Severity is where the distinction either survives or is lost: absent is a
/// warning, and both "resolved" and "not followed" are information. If the
/// third ever became a warning, this crate would be reporting an absence of
/// evidence as a defect.
#[test]
fn the_report_keeps_the_three_outcomes_apart() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture("conformant-dangling-references.yaml"));

    assert_eq!(
        support::count_code(&report, codes::REFERENCE_RESOLVED),
        2,
        "expected both resolving references to be recorded: {:?}",
        support::codes_in(&report)
    );
    assert_eq!(
        support::count_code(&report, codes::REFERENCE_DOES_NOT_EXIST),
        1
    );
    assert_eq!(
        support::count_code(&report, codes::DEFINITION_DOES_NOT_EXIST),
        1
    );
    assert_eq!(
        support::count_code(&report, codes::REFERENCE_NOT_INSPECTED),
        2,
        "expected the external `$ref` and the array-element reference: {:?}",
        support::codes_in(&report)
    );

    for diagnostic in &report {
        let severity = match diagnostic.code.as_str() {
            c if c == codes::REFERENCE_DOES_NOT_EXIST || c == codes::DEFINITION_DOES_NOT_EXIST => {
                Severity::Warning
            }
            c if c == codes::REFERENCE_RESOLVED || c == codes::REFERENCE_NOT_INSPECTED => {
                Severity::Info
            }
            _ => continue,
        };
        assert_eq!(
            diagnostic.severity, severity,
            "{diagnostic} is at the wrong severity for its outcome"
        );
    }

    // And none of it changes the verdict: the schema says nothing about where
    // any of these point, so neither does this crate's pass/fail.
    assert!(
        support::errors(&report).is_empty(),
        "a cross-reference finding became an error, which would make this crate fail documents \
         the published schema accepts: {:?}",
        support::errors(&report)
    );
}

/// `Resolution` is `conform-core`'s, and is used as such.
///
/// A local three-way enum would satisfy every assertion above. This one does
/// not compile unless the crate really hands back the shared type.
#[test]
fn the_resolvers_hand_back_conform_cores_resolution() {
    let document = document();
    let resolution: Resolution<conform_core::Pointer> =
        resolve_ref(&document, "#/definitions/customer_id");
    assert!(matches!(resolution, Resolution::Resolved(_)));

    let absent: Resolution<conform_core::Pointer> = resolve_ref(&document, "#/definitions/absent");
    assert_eq!(absent, Resolution::DoesNotExist);

    let unknown: Resolution<conform_core::Pointer> =
        resolve_ref(&document, "https://example.org/x");
    assert_eq!(
        unknown,
        Resolution::not_inspected(NotInspectedReason::OutsideDocumentSet)
    );

    // The two non-resolving outcomes are different values, and stay different.
    assert_ne!(absent, unknown);
}
