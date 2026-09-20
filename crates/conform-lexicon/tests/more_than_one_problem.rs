//! The capability `Result<(), String>` cannot express.
//!
//! Everything else this crate adds — codes, severities, pointers, help text —
//! could in principle be smuggled into a cleverly formatted `String`. This one
//! cannot. A `Result<(), String>` holds exactly one `Err`, so a document with
//! six faults reports one, and the five others are discovered one round trip
//! at a time.
//!
//! The pre-existing `validate_odcl_internal` is worse than that bound
//! suggests, because the `jsonschema` call underneath it short-circuits:
//!
//! ```text
//! if let Err(error) = validator.validate(&data) {
//!     let error_msg = format_validation_error(&error, "ODCL");
//!     return Err(error_msg);
//! }
//! ```
//!
//! `validate` yields the *first* error. This crate calls `iter_errors`, which
//! yields all of them. The recorded oracle output makes the difference
//! concrete: for `faulty-many-faults.yaml` the oracle's entire output is
//!
//! ```text
//! ODCL validation failed at path '/dataContractSpecification': "2.0.0" is not one of …
//! ```
//!
//! and nothing else. That one line is also the *least* useful of the six
//! findings in that document: the reader fixes the version, re-runs, and
//! learns that `id` is a number.

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
        errors.len() >= 6,
        "expected at least six separate errors from a document built to carry six independent \
         faults; got {}:\n  {}",
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
    // validator that reported the same missing-property finding six times
    // would satisfy the count assertion above and still be telling the reader
    // one thing.
    for expected in [
        conform_lexicon::codes::REQUIRED_MISSING,
        conform_lexicon::codes::VALUE_NOT_PERMITTED,
        conform_lexicon::codes::WRONG_TYPE,
        conform_lexicon::codes::MALFORMED_VALUE,
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
        // A missing required property at the ROOT genuinely applies to the
        // whole document, so it carries no pointer. Everything else must
        // point somewhere — including `REQUIRED_MISSING` inside `info`, which
        // is why this is not a blanket exemption for the code.
        let root_level = diagnostic.code.as_str() == conform_lexicon::codes::REQUIRED_MISSING;
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
        assert_eq!(spec.id, conform_lexicon::SPEC_ID);
        assert_eq!(
            spec.version.as_deref(),
            Some("1.2.1"),
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

/// Reporting and gating stay separate, which is `conform-core`'s central
/// convention and the one an adapter is most likely to erode.
#[test]
fn reporting_everything_is_not_the_same_as_failing() {
    use conform_core::{GatePolicy, GateVerdict};

    let validator = support::validator();

    // A document with warnings and no errors reports plenty and gates on
    // nothing — until a policy is asked for that says otherwise.
    let report = validator.validate(&support::fixture("conformant-open-root-key.yaml"));
    assert!(
        report.len() > 1,
        "nothing was reported, so nothing is being demonstrated"
    );
    assert!(!report.should_gate(GatePolicy::default()));
    assert!(report.should_gate(GatePolicy::warnings_as_errors()));

    // And `check` is the same decision, made once, through the trait.
    assert_eq!(
        validator.check(
            &support::fixture("conformant-open-root-key.yaml"),
            GatePolicy::default()
        ),
        GateVerdict::Passed {
            worst: Some(Severity::Warning)
        }
    );
    assert!(
        validator
            .check(&support::fixture(MANY_FAULTS), GatePolicy::default())
            .is_gated()
    );
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

    // Not empty, though, and that is the point: "checked, and correct" must
    // not look like "not checked". Four notes — the provenance note, the two
    // references that were followed and found, and the one server the schema's
    // own defect stopped this run from checking properly.
    assert_eq!(report.count(Severity::Info), report.len());
    assert_eq!(report.len(), 4, "{:?}", support::codes_in(&report));
}
