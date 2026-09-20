//! Reporting says what is true. Gating says what fails. They are different
//! decisions, and this binary must not conflate them.
//!
//! `conform-core` models the split with `GatePolicy`, every adapter in this
//! family is built around it, and the command line is the one place a user
//! meets it. A `conform validate` that exited non-zero because it *found*
//! something would undo the whole design at the last step — and it is an easy
//! mistake to make, because "the tool printed errors, so it should fail" reads
//! as common sense right up until a warning fails somebody's unrelated pull
//! request and the gate gets switched off for good.

mod support;

use support::{conform, conform_json, in_workspace};

/// A document with three schema errors and four hygiene warnings.
fn faulty() -> String {
    in_workspace("crates/conform-odcs/tests/fixtures/faulty-missing-required.yaml")
}

/// A document the schema accepts, with hygiene warnings and nothing worse.
fn clean_but_sparse() -> String {
    in_workspace("crates/conform-odcs/tests/fixtures/conformant-minimal.yaml")
}

#[test]
fn a_bare_validate_reports_everything_and_exits_zero() {
    let output = conform(&["validate", &faulty()]);

    assert_eq!(
        output.code, 0,
        "reporting is not gating:\n{}",
        output.stdout
    );
    assert!(output.stdout.contains("error ODCS101"));
    assert!(output.stdout.contains("found: 3 error(s)"));
    // And it says so, rather than leaving a reader to wonder whether the
    // errors were somehow not real.
    assert!(output.stdout.contains("gate:  none"));
}

#[test]
fn gating_is_what_makes_findings_fail() {
    let document = faulty();
    for gate in [vec!["--gate", "errors"], vec!["--check"]] {
        let mut args = vec!["validate", document.as_str()];
        args.extend(gate.iter().copied());

        let output = conform(&args);
        assert_eq!(output.code, 1, "{}", output.stdout);
        assert!(output.stdout.contains("GATED"));
    }
}

#[test]
fn a_warning_only_document_passes_an_error_gate_and_fails_a_warning_gate() {
    // The distinction the two thresholds exist for. Same document, same
    // findings, two different answers to "should this fail the build".
    let errors = conform(&["validate", &clean_but_sparse(), "--gate", "errors"]);
    assert_eq!(errors.code, 0, "{}", errors.stdout);

    let warnings = conform(&["validate", &clean_but_sparse(), "--gate", "warnings"]);
    assert_eq!(warnings.code, 1, "{}", warnings.stdout);

    // And the findings themselves are identical. Only the verdict moved.
    let (lenient, _) = conform_json(&["validate", &clean_but_sparse(), "--gate", "errors"]);
    let (strict, _) = conform_json(&["validate", &clean_but_sparse(), "--gate", "warnings"]);
    assert_eq!(lenient["diagnostics"], strict["diagnostics"]);
    assert_ne!(lenient["gate"], strict["gate"]);
}

#[test]
fn a_run_that_could_not_happen_exits_two() {
    // Distinguishable from both of the above, because "I reached no verdict"
    // is not "I reached a verdict and it was fine", and a CI job that treats
    // them alike will one day go green on a tool that never ran.
    let document = faulty();
    for args in [
        vec!["validate", document.as_str(), "--spec", "nonesuch"],
        vec!["validate", document.as_str(), "--spec", "cads"],
    ] {
        let output = conform(&args);
        assert_eq!(output.code, 2, "{}", output.stdout);
        assert!(output.stdout.contains("exit:  2"));
    }

    let missing = conform(&["registry", "list", "--registry", "no/such/specs.toml"]);
    assert_eq!(missing.code, 2, "{}", missing.stdout);
}

#[test]
fn a_catalogued_specification_with_no_validator_is_said_so_rather_than_passed() {
    // `odcl` and `cads` are in the registry and have no adapter here. The
    // dangerous answer is a clean run over zero documents; the honest one is a
    // refusal that names the gap.
    let output = conform(&["validate", &faulty(), "--spec", "odcl"]);
    assert_eq!(output.code, 2);
    assert!(output.stdout.contains("CLI005"));

    let (json, _) = conform_json(&["registry", "list", "--spec", "odcl"]);
    assert_eq!(json["specs"][0]["has_validator"], serde_json::json!(false));
}

#[test]
fn paths_that_hold_nothing_checkable_are_an_error_and_not_a_silence() {
    // The failure this guards against: pointing the tool at the wrong
    // directory, getting a clean run, and believing something was checked.
    let empty = support::scratch("nothing-to-check");
    support::write(&empty, "notes.txt", "nothing here is a specification\n");

    let output = conform(&["validate", &empty.display().to_string()]);
    assert!(output.stdout.contains("CLI003"));
    assert!(output.stdout.contains("found: 1 error(s)"));

    // Still exit 0 without a gate — the split holds even here — and 1 with one.
    assert_eq!(output.code, 0);
    let gated = conform(&["validate", &empty.display().to_string(), "--check"]);
    assert_eq!(gated.code, 1);
}

#[test]
fn a_file_naming_no_standard_is_reported_rather_than_skipped() {
    let directory = support::scratch("unrecognised");
    let odcl = support::write(
        &directory,
        "legacy.yaml",
        "dataContractSpecification: 1.1.0\nid: urn:datacontract:retail:orders\n",
    );

    let output = conform(&["validate", &odcl]);
    assert!(output.stdout.contains("CLI002"), "{}", output.stdout);
    assert!(output.stdout.contains("no `kind` key"), "{}", output.stdout);
}
