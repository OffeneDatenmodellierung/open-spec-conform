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

/// An ODCL document with six schema errors and four hygiene warnings.
fn faulty_lexicon() -> String {
    in_workspace("crates/conform-lexicon/tests/fixtures/faulty-many-faults.yaml")
}

/// An ODCL document the schema accepts, with hygiene warnings and nothing
/// worse.
fn clean_but_sparse_lexicon() -> String {
    in_workspace("crates/conform-lexicon/tests/fixtures/conformant-minimal.yaml")
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
        vec!["validate", document.as_str(), "--spec", "ossie"],
        vec!["validate", document.as_str(), "--spec", "ossie-dev"],
    ] {
        let output = conform(&args);
        assert_eq!(output.code, 2, "{}", output.stdout);
        assert!(output.stdout.contains("exit:  2"));
    }

    let missing = conform(&["registry", "list", "--registry", "no/such/specs.toml"]);
    assert_eq!(missing.code, 2, "{}", missing.stdout);
}

/// Every catalogued specification this binary has no adapter for.
///
/// Listed rather than derived, and deliberately so: `Standard::from_spec_id`
/// is the very function under test, so asking it which entries lack an adapter
/// would make this test agree with whatever that function currently says. A
/// specification that grows a validator has to be deleted from here by hand,
/// which is the same deliberate act `odcl` already required once.
const CATALOGUED_WITHOUT_AN_ADAPTER: [&str; 2] = ["cads", "ossie"];

#[test]
fn a_catalogued_specification_with_no_validator_is_said_so_rather_than_passed() {
    // Each of these is in the registry and has no adapter here. The dangerous
    // answer is a clean run over zero documents; the honest one is a refusal
    // that names the gap.
    for id in CATALOGUED_WITHOUT_AN_ADAPTER {
        let output = conform(&["validate", &faulty(), "--spec", id]);
        assert_eq!(output.code, 2, "`--spec {id}`: {}", output.stdout);
        assert!(
            output.stdout.contains("CLI005"),
            "`--spec {id}` did not refuse under CLI005: {}",
            output.stdout
        );

        let (json, _) = conform_json(&["registry", "list", "--spec", id]);
        assert_eq!(
            json["specs"][0]["id"],
            serde_json::json!(id),
            "`registry list --spec {id}` selected a different entry"
        );
        assert_eq!(
            json["specs"][0]["has_validator"],
            serde_json::json!(false),
            "`{id}` is advertised as having a validator it does not have"
        );
    }
}

#[test]
fn a_catalogued_specification_without_an_adapter_is_still_catalogued_and_verified() {
    // The other half of the refusal above, and the reason the refusal is not
    // simply "unknown specification": these entries are vendored, hashed and
    // re-verified like every other. What is missing is an adapter, and the
    // console has to show both facts at once or `--spec ossie` reads as "we
    // have never heard of it".
    let (json, code) = conform_json(&["registry", "verify"]);
    assert_eq!(code, 0);

    for id in CATALOGUED_WITHOUT_AN_ADAPTER {
        let spec = json["specs"]
            .as_array()
            .expect("specs is an array")
            .iter()
            .find(|spec| spec["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("`{id}` is not in the catalogue at all"));

        assert_eq!(spec["vendored"]["verify"], serde_json::json!("matched"));
        assert_eq!(spec["has_validator"], serde_json::json!(false));
    }
}

#[test]
fn odcl_is_catalogued_and_now_has_a_validator_and_says_both() {
    // The other half of the test above, and the reason it had to change.
    // `odcl` was catalogued-without-an-adapter and is no longer: a console
    // that still refused `--spec odcl` would be showing a specification it
    // could in fact verify.
    let (json, code) = conform_json(&["registry", "list", "--spec", "odcl"]);
    assert_eq!(code, 0);
    assert_eq!(json["specs"][0]["id"], serde_json::json!("odcl"));
    assert_eq!(json["specs"][0]["has_validator"], serde_json::json!(true));

    // And it validates, rather than refusing under `CLI005`.
    let output = conform(&["validate", &faulty_lexicon(), "--spec", "odcl"]);
    assert_eq!(output.code, 0, "{}", output.stdout);
    assert!(!output.stdout.contains("CLI005"), "{}", output.stdout);
    assert!(output.stdout.contains("error ODCL103"), "{}", output.stdout);
}

#[test]
fn an_odcl_document_reports_without_gating_and_gates_when_asked() {
    // The central commitment, held at the one place a user meets it, for the
    // adapter that has just been wired in. A new adapter is exactly where this
    // gets broken, because "it found errors, so it should fail" reads as
    // common sense.
    let document = faulty_lexicon();

    let bare = conform(&["validate", &document]);
    assert_eq!(bare.code, 0, "reporting is not gating:\n{}", bare.stdout);
    assert!(bare.stdout.contains("found: 6 error(s)"), "{}", bare.stdout);
    assert!(bare.stdout.contains("gate:  none"), "{}", bare.stdout);

    for gate in [vec!["--gate", "errors"], vec!["--check"]] {
        let mut args = vec!["validate", document.as_str()];
        args.extend(gate.iter().copied());

        let output = conform(&args);
        assert_eq!(output.code, 1, "{}", output.stdout);
        assert!(output.stdout.contains("GATED"), "{}", output.stdout);
    }

    // And the two thresholds still disagree about a warning-only ODCL
    // document, which is what makes them two thresholds.
    let sparse = clean_but_sparse_lexicon();
    assert_eq!(conform(&["validate", &sparse, "--gate", "errors"]).code, 0);
    assert_eq!(
        conform(&["validate", &sparse, "--gate", "warnings"]).code,
        1
    );
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
    let stranger = support::write(
        &directory,
        "deployment.yaml",
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: orders\n",
    );

    let output = conform(&["validate", &stranger]);
    assert!(output.stdout.contains("CLI002"), "{}", output.stdout);
    assert!(
        output
            .stdout
            .contains("`kind: Deployment` names no standard"),
        "{}",
        output.stdout
    );

    // A file with neither discriminator is reported under the same code, and
    // the message names both of the things it looked for.
    let anonymous = support::write(&directory, "notes.yaml", "title: Orders\nowner: platform\n");
    let second = conform(&["validate", &anonymous]);
    assert!(second.stdout.contains("CLI002"), "{}", second.stdout);
    assert!(
        second
            .stdout
            .contains("no `kind` key and no `dataContractSpecification` key"),
        "{}",
        second.stdout
    );
}

#[test]
fn a_document_carrying_the_lexicon_root_key_is_routed_and_not_reported_as_a_stranger() {
    // The exact document this test used to assert was unrecognised. It is a
    // legacy data contract, `conform-lexicon` validates it, and reporting it
    // as "nothing here can tell what this is" would now be a lie.
    let directory = support::scratch("routed-lexicon");
    let legacy = support::write(
        &directory,
        "legacy.yaml",
        "dataContractSpecification: 1.1.0\nid: urn:datacontract:retail:orders\n",
    );

    let output = conform(&["validate", &legacy]);
    assert!(!output.stdout.contains("CLI002"), "{}", output.stdout);
    assert!(output.stdout.contains("(odcl)"), "{}", output.stdout);
    // `info` is required and absent, so the adapter has something to say —
    // which is the point of routing it somewhere rather than nowhere.
    assert!(output.stdout.contains("error ODCL101"), "{}", output.stdout);
    // Still exit 0 without a gate.
    assert_eq!(output.code, 0, "{}", output.stdout);
}
