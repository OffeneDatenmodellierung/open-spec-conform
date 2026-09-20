//! What crosses the boundary is a JSON report, and it is JSON-encoded exactly
//! once.
//!
//! # Two halves of one rule
//!
//! Every adapter in this family quotes the document it found a fault in, and
//! none of them sanitises what it quotes — escaping is not idempotent, so it
//! has to happen exactly once, at the point of *display*.
//!
//! `conform-cli` is a display: its human renderer and its TUI neutralise
//! U+202E, `ESC`, `BEL` and zero-width runs, and
//! `crates/conform-cli/tests/hostile_text_is_neutralised.rs` holds them to it.
//! Its `--json` sink deliberately does not, and the same file holds it to
//! that too.
//!
//! **This crate is the second `--json`.** It is a library boundary, not a
//! terminal; JSON string encoding is already the correct and complete escaping
//! here, and applying a terminal's neutralisation on top would hand a consumer
//! `<U+202E>` where the document held one character. That is data corruption
//! with a reassuring shape, which is the worst kind.
//!
//! So this file asserts the *negative*: the payload arrives intact. The test
//! that would catch a well-meaning future change is this one, because a
//! terminal-escaping call added here would look like a safety improvement in
//! review.

#![allow(
    unsafe_code,
    reason = "calling a C ABI from Rust is unsafe by construction; a test of \
              that ABI that could not write `unsafe` would be a test of \
              something else"
)]

mod support;

use conform_ffi::{ConformStatus, abi};
use serde_json::Value;

use support::{HOSTILE, hostile_contract, last_error, release, validate, validator};

/// Parse the boundary's output, insisting it is JSON.
fn envelope(json: &str) -> Value {
    serde_json::from_str(json).unwrap_or_else(|error| {
        panic!("the boundary handed back something that is not JSON: {error}\n{json}")
    })
}

#[test]
fn a_conformant_contract_crosses_with_no_errors() {
    let handle = validator("odcs");
    let document = std::fs::read(support::in_workspace(
        "crates/conform-odcs/tests/fixtures/conformant-minimal.yaml",
    ))
    .expect("the fixture is in the repository");

    let (status, json) = validate(handle, "conformant-minimal.yaml", &document);
    release(handle);

    assert_eq!(status, ConformStatus::Ok, "{}", last_error());
    let report = envelope(&json.expect("a successful call returns a report"));

    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["tool"]["name"], "conform-ffi");
    assert_eq!(report["spec"]["id"], "odcs");
    assert_eq!(report["document"]["id"], "conformant-minimal.yaml");
    assert_eq!(
        report["document"]["error"], 0,
        "a conformant fixture produced errors: {report}"
    );
}

#[test]
fn a_faulty_contract_crosses_as_a_success_carrying_errors() {
    // The distinction the status code exists to draw. `Ok` means *a report was
    // produced*; whether the document conformed is in the report. Reporting
    // and gating are separate decisions in this family, and this boundary does
    // not gate at all.
    let handle = validator("odcs");
    let document = std::fs::read(support::in_workspace(
        "crates/conform-odcs/tests/fixtures/faulty-many-faults.yaml",
    ))
    .expect("the fixture is in the repository");

    let (status, json) = validate(handle, "faulty-many-faults.yaml", &document);
    release(handle);

    assert_eq!(
        status,
        ConformStatus::Ok,
        "a document full of errors is not a failure of the call"
    );
    let report = envelope(&json.expect("a successful call returns a report"));

    assert!(
        report["document"]["error"].as_u64().unwrap_or(0) > 0,
        "the fixture named `faulty-many-faults` produced none: {report}"
    );
    assert_eq!(report["document"]["worst_severity"], "error");

    // Every finding carries the three things a consumer needs to act: a stable
    // code to match on, a severity to rank by, and somewhere to look.
    let diagnostics = report["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    assert!(!diagnostics.is_empty());
    for finding in diagnostics {
        assert!(finding["code"].as_str().is_some_and(|c| !c.is_empty()));
        assert!(matches!(
            finding["severity"].as_str(),
            Some("error" | "warning" | "info")
        ));
        assert_eq!(finding["location"]["document"], "faulty-many-faults.yaml");
    }
}

#[test]
fn the_counts_agree_with_the_findings_they_count() {
    let handle = validator("odps");
    let document = std::fs::read(support::in_workspace(
        "crates/conform-odps/tests/fixtures/faulty-missing-required.yaml",
    ))
    .expect("the fixture is in the repository");

    let (status, json) = validate(handle, "faulty-missing-required.yaml", &document);
    release(handle);

    assert_eq!(status, ConformStatus::Ok, "{}", last_error());
    let report = envelope(&json.expect("a successful call returns a report"));
    let diagnostics = report["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");

    let count = |severity: &str| {
        diagnostics
            .iter()
            .filter(|d| d["severity"] == severity)
            .count()
    };

    // A summary that disagrees with the list under it is worse than no summary
    // at all, because a consumer will believe the cheap one.
    assert_eq!(report["document"]["diagnostics"], diagnostics.len());
    assert_eq!(report["document"]["error"], count("error"));
    assert_eq!(report["document"]["warning"], count("warning"));
    assert_eq!(report["document"]["info"], count("info"));
}

#[test]
fn the_payload_really_does_reach_a_diagnostic() {
    // The negative control for the test below. If the adapter stopped quoting
    // the document's `status`, that test would pass while proving nothing.
    let handle = validator("odcs");
    let (status, json) = validate(handle, "orders.yaml", hostile_contract().as_bytes());
    release(handle);

    assert_eq!(status, ConformStatus::Ok, "{}", last_error());
    let report = envelope(&json.expect("a successful call returns a report"));

    let carrying = report["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .filter(|d| d["message"].as_str().is_some_and(|m| m.contains(HOSTILE)))
        .count();

    assert_eq!(
        carrying, 1,
        "expected exactly one diagnostic quoting the document's `status`; \
         the adapter's hygiene rule may have stopped interpolating it: {report}"
    );
}

#[test]
fn the_boundary_hands_over_the_bytes_the_document_really_held() {
    let handle = validator("odcs");
    let (_, json) = validate(handle, "orders.yaml", hostile_contract().as_bytes());
    release(handle);

    let json = json.expect("a successful call returns a report");

    // JSON encoding, and only JSON encoding: the raw string the boundary
    // handed over must not contain a literal `ESC` or `BEL`, because
    // `serde_json` escapes control characters — but the *decoded* message must
    // hold the real characters.
    assert!(
        !json.contains('\u{1b}') && !json.contains('\u{7}'),
        "a raw control character survived into the JSON text, so it was not \
         JSON-encoded at all"
    );

    let report = envelope(&json);
    let messages: Vec<String> = report["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .filter_map(|finding| finding["message"].as_str())
        .map(ToOwned::to_owned)
        .collect();

    // Found by an *ordinary* run of characters from inside the payload — the
    // window title the OSC sequence sets — so that the search cannot be the
    // assertion. `pwned` survives every escaping anyone might apply, appears
    // nowhere else in a report, and locating a message by it says nothing
    // about whether the hostile characters around it made it across. That is
    // the next line's job.
    let quoting = messages
        .iter()
        .find(|message| message.contains("pwned"))
        .expect("a diagnostic quoting the document's `status`");
    assert!(
        quoting.contains(HOSTILE),
        "the payload was altered on the way across: {}",
        quoting.escape_default()
    );

    // And specifically: the terminal escaping `conform-cli` applies must not
    // have been applied here. A consumer reading `<U+202E>` where the document
    // held one character has been handed corrupted data.
    for terminal_escape in ["<U+202E>", "<U+001B>", "<U+0007>", "<U+200B>"] {
        for message in &messages {
            assert!(
                !message.contains(terminal_escape),
                "{terminal_escape} appears in a message, so terminal escaping \
                 has leaked into a sink that is not a terminal"
            );
        }
    }
}

#[test]
fn the_version_is_the_crate_version_and_is_a_static_string() {
    let pointer = abi::conform_version();
    assert!(!pointer.is_null());

    // SAFETY: the API documents this as a NUL-terminated static string with
    // the lifetime of the process, which is exactly what makes the second read
    // below legitimate.
    let first = unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_str()
        .expect("UTF-8");
    assert_eq!(first, env!("CARGO_PKG_VERSION"));

    // Called twice, and neither call freed anything: a caller that treats this
    // like `conform_validate`'s output and frees it would be following the
    // wrong half of the API, so the contract that it is static had better hold.
    assert_eq!(abi::conform_version(), pointer);
}

#[test]
fn the_three_version_numbers_are_three_numbers() {
    // The ABI's version, the library's version and the envelope's version
    // answer different questions and move on different occasions. Asserting
    // they are separately reachable is what stops a future tidy-up collapsing
    // them into one.
    assert_eq!(conform_ffi::CONFORM_ABI_VERSION, 1);
    assert_eq!(conform_ffi::SCHEMA_VERSION, 1);

    let handle = validator("odcs");
    let (_, json) = validate(handle, "orders.yaml", b"version: 1.0.0\n");
    release(handle);

    let report = envelope(&json.expect("a report"));
    assert_eq!(report["schema_version"], conform_ffi::SCHEMA_VERSION);
    assert_eq!(report["tool"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn a_validator_reports_the_standard_it_speaks_for() {
    for (spec_id, fixture) in [
        (
            "odcs",
            "crates/conform-odcs/tests/fixtures/conformant-minimal.yaml",
        ),
        (
            "odps",
            "crates/conform-odps/tests/fixtures/conformant-minimal.yaml",
        ),
    ] {
        let handle = validator(spec_id);
        let document = std::fs::read(support::in_workspace(fixture)).expect("fixture");
        let (status, json) = validate(handle, fixture, &document);
        release(handle);

        assert_eq!(status, ConformStatus::Ok, "{}", last_error());
        let report = envelope(&json.expect("a report"));
        assert_eq!(report["spec"]["id"], spec_id);
        assert!(
            report["spec"]["version"].is_string(),
            "the registry records a version for `{spec_id}`, so the report \
             should carry it: {report}"
        );
    }
}
