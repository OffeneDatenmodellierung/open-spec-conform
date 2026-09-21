//! The capability `Result<(), String>` cannot express.
//!
//! Everything else this crate adds — codes, severities, pointers, help text —
//! could in principle be smuggled into a cleverly formatted `String`. This one
//! cannot. A `Result<(), String>` holds exactly one `Err`, so a document with
//! twelve faults reports one, and the eleven others are discovered one round
//! trip at a time.
//!
//! The pre-existing `validate_odps_internal` is worse than that bound
//! suggests, because the `jsonschema` call underneath it short-circuits:
//!
//! ```text
//! if let Err(error) = validator.validate(&data) {
//!     return Err(format_validation_error(&error, "ODPS"));
//! }
//! ```
//!
//! `validate` yields the *first* error. This crate calls `iter_errors`, which
//! yields all of them. The recorded oracle output makes the difference
//! concrete: for `faulty-many-faults.yaml` the oracle's entire output is
//!
//! ```text
//! ODPS validation failed at path '/apiVersion': "v2.0.0" is not one of …
//! ```
//!
//! and nothing else.

mod support;

use std::collections::BTreeSet;

use conform_core::{Severity, Validator};

/// The fixture is built to carry faults of several different classes; see the
/// comment at the top of it.
const MANY_FAULTS: &str = "faulty-many-faults.yaml";

#[test]
fn a_document_with_many_faults_reports_many_errors() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture(MANY_FAULTS));
    let errors: Vec<&conform_core::Diagnostic> = report.at_or_above(Severity::Error).collect();

    assert!(
        errors.len() >= 8,
        "expected at least eight separate errors from a document built to carry eight \
         independent faults; got {}:\n  {}",
        errors.len(),
        support::errors(&report).join("\n  ")
    );
}

#[test]
fn those_errors_are_of_several_different_kinds() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture(MANY_FAULTS));

    let codes: BTreeSet<&str> = report
        .at_or_above(Severity::Error)
        .map(|d| d.code.as_str())
        .collect();

    // Not merely "more than one error" — more than one *kind* of error. A
    // validator that reported the same missing-property finding eight times
    // would satisfy the count assertion above and still be telling the reader
    // one thing.
    for expected in [
        conform_odps::codes::REQUIRED_MISSING,
        conform_odps::codes::UNKNOWN_PROPERTY,
        conform_odps::codes::VALUE_NOT_PERMITTED,
        conform_odps::codes::WRONG_TYPE,
    ] {
        assert!(
            codes.contains(expected),
            "no finding under {expected}; the codes raised were {codes:?}"
        );
    }
}

#[test]
fn every_error_says_where_it_is() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture(MANY_FAULTS));

    for diagnostic in report.at_or_above(Severity::Error) {
        assert_eq!(
            diagnostic.location.document.as_str(),
            MANY_FAULTS,
            "a diagnostic named a document other than the one under test"
        );
        // Root-level findings — a missing required property, a key the closed
        // root does not permit — genuinely apply to the whole document, so
        // they carry no pointer. Everything else must point somewhere.
        let root_level = matches!(
            diagnostic.code.as_str(),
            conform_odps::codes::REQUIRED_MISSING | conform_odps::codes::UNKNOWN_PROPERTY
        );
        assert!(
            diagnostic.location.pointer.is_some() || root_level,
            "{} carries no pointer: {diagnostic}",
            diagnostic.code
        );
    }

    // And at least some of them are genuinely nested, so the pointers are
    // doing work rather than all naming the root.
    let nested = report
        .at_or_above(Severity::Error)
        .filter(|d| {
            d.location
                .pointer
                .as_ref()
                .is_some_and(|p| p.as_str().matches('/').count() >= 2)
        })
        .count();
    assert!(
        nested >= 1,
        "no finding pointed inside a nested structure, so the pointers are not discriminating:\n  {}",
        support::errors(&report).join("\n  ")
    );
}

#[test]
fn every_error_carries_the_standard_it_was_raised_under() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture(MANY_FAULTS));

    for diagnostic in report.at_or_above(Severity::Error) {
        let spec = diagnostic
            .spec_ref
            .as_ref()
            .unwrap_or_else(|| panic!("{} carries no spec reference", diagnostic.code));
        assert_eq!(spec.id, conform_odps::SPEC_ID);
        assert_eq!(
            spec.version.as_deref(),
            Some("1.0.0"),
            "a finding did not say which version of the standard objected"
        );
        assert!(
            spec.section.is_some(),
            "{} does not name the schema clause that objected, so a reader cannot get from the \
             finding back to the text that raised it",
            diagnostic.code
        );
    }
}

/// The control. If the hygiene rules or the schema mapping fired
/// indiscriminately, every test above would pass and mean nothing.
#[test]
fn a_clean_document_reports_no_errors_and_no_warnings() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture("conformant-full.yaml"));

    assert!(
        report.at_or_above(Severity::Warning).next().is_none(),
        "a document written to be exemplary produced findings:\n  {}",
        report
            .at_or_above(Severity::Warning)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    // Not empty, though: the provenance note is always there, and that is the
    // point of it — "checked, and correct" must not look like "not checked".
    assert_eq!(report.len(), 1);
    assert_eq!(report.count(Severity::Info), 1);
}
