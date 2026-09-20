//! Differential test: does this crate reach the **same verdict** as the
//! validator it replaces?
//!
//! # What is being claimed
//!
//! `data-modelling-sdk`'s `validate_odcs_internal` returns `Result<(), String>`.
//! The claim this crate makes is narrow and testable: *same verdict, richer
//! detail*. A document that function says is fine, this crate passes. A
//! document it rejects, this crate gates. What changes is that one `String`
//! becomes many diagnostics with codes, severities and locations.
//!
//! This file is where that claim is held to account over the whole fixture
//! corpus, in both directions — a false pass and a false fail are both
//! failures here.
//!
//! # Why the oracle is a recording rather than a live call
//!
//! The honest answer, because it matters for how much this test is worth.
//!
//! The oracle **was actually run**. `tools/oracle` is a small binary that
//! depends on `data-modelling-core` by path, with `--features
//! schema-validation`, and calls `validate_odcs_internal` directly over every
//! file in `tests/fixtures/`. What it returned is committed verbatim in
//! `tests/oracle/odcs-verdicts.json`. Nothing here is a reimplementation of
//! the oracle's rules and nothing here is guessed.
//!
//! What this test cannot do is call it *from inside the workspace*, because a
//! dev-dependency on the SDK puts `yaml-rust 0.4.5` into the dependency graph
//! `cargo deny` gates on:
//!
//! ```text
//! error[unmaintained]: yaml-rust is unmaintained.
//! ├ ID: RUSTSEC-2024-0320
//! ├ Solution: No safe upgrade is available!
//! ```
//!
//! That is a true finding about the SDK. Silencing it in `deny.toml` to make a
//! test compile would trade a real supply-chain signal for a convenience, so
//! the recording is the compromise — and it is guarded:
//!
//! - every fixture on disk must have a record, and every record a fixture, so
//!   neither side can grow an entry the other has not seen;
//! - every record carries the **SHA-256 of the fixture it was taken from**, and
//!   this test re-hashes the file. Edit a fixture without re-running the
//!   recorder and this test fails rather than comparing against a stale
//!   verdict;
//! - the recording carries the digest of the schema the oracle read and the
//!   digest of the schema this crate reads, and this test asserts they are the
//!   same bytes — so a disagreement is always about *rules*, never about two
//!   validators having been handed different schemas.
//!
//! Regenerate with `cargo run --manifest-path tools/oracle/Cargo.toml`.
//!
//! # Disagreements are recorded, not smoothed over
//!
//! [`KNOWN_DIVERGENCES`] lists every fixture where the two verdicts differ,
//! each with the reason. It is not a suppression list, and the difference is
//! enforced: a listed fixture that stops diverging **fails this test**. So the
//! list cannot quietly become an exemption for a regression, and it has to be
//! revisited the moment either side changes.

mod support;

use std::collections::BTreeMap;
use std::fs;

use conform_core::Validator;
use conform_registry::sha256_hex;
use serde_json::Value;

/// Every fixture on which this crate and the oracle reach different verdicts,
/// and why.
///
/// One entry today, and it is a finding about the oracle rather than about
/// this crate — see the rationale.
const KNOWN_DIVERGENCES: &[(&str, &str)] = &[(
    "sniffed-odcl-specification-key.yaml",
    "The oracle returns Ok(()). `validate_odcs_internal` sniffs its input \
     before validating: a document carrying a `dataContractSpecification` key \
     is silently handed to the ODCL validator instead, and ODCL's verdict is \
     returned as though ODCS had been checked. This fixture is a well-formed \
     ODCL document, so that path passes it. The consequence is that a caller \
     running `validate_odcs_internal` across a directory gets a green light on \
     a file that is not an ODCS contract at all. This crate does not sniff: \
     asked for an ODCS verdict it gives one, and this document is not a \
     conformant ODCS contract. Recorded as a finding against the SDK; not \
     something for this crate to imitate.",
)];

/// The oracle's recording, parsed.
fn recording() -> Value {
    let path = support::fixtures_dir()
        .parent()
        .expect("tests/fixtures has a parent")
        .join("oracle/odcs-verdicts.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// `fixture name -> (verdict, recorded sha256)`.
fn recorded_verdicts(recording: &Value) -> BTreeMap<String, (bool, String)> {
    recording["fixtures"]
        .as_array()
        .expect("the recording has a `fixtures` array")
        .iter()
        .map(|record| {
            let name = record["fixture"]
                .as_str()
                .expect("a record names its fixture")
                .to_owned();
            let verdict = match record["verdict"].as_str() {
                Some("ok") => false,
                Some("err") => true,
                other => panic!("{name}: unrecognised recorded verdict {other:?}"),
            };
            let sha = record["sha256"]
                .as_str()
                .expect("a record carries the fixture's digest")
                .to_owned();
            (name, (verdict, sha))
        })
        .collect()
}

#[test]
fn the_recording_covers_exactly_the_corpus() {
    let recording = recording();
    let recorded = recorded_verdicts(&recording);

    let on_disk: Vec<String> = support::fixtures()
        .iter()
        .map(|p| {
            p.file_name()
                .expect("a file has a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    let missing: Vec<&String> = on_disk
        .iter()
        .filter(|n| !recorded.contains_key(*n))
        .collect();
    assert!(
        missing.is_empty(),
        "these fixtures have no recorded oracle verdict, so this test would silently skip them. \
         Re-run `cargo run --manifest-path tools/oracle/Cargo.toml`.\n  {missing:?}"
    );

    let extra: Vec<&String> = recorded.keys().filter(|n| !on_disk.contains(n)).collect();
    assert!(
        extra.is_empty(),
        "the recording holds verdicts for fixtures that are no longer on disk: {extra:?}"
    );

    assert!(
        !on_disk.is_empty(),
        "the corpus is empty, so every assertion in this file would pass vacuously"
    );
}

#[test]
fn the_recording_is_not_stale() {
    let recording = recording();
    let recorded = recorded_verdicts(&recording);

    let mut drifted = Vec::new();
    for path in support::fixtures() {
        let name = path
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let bytes =
            fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let actual = sha256_hex(&bytes);
        if let Some((_, recorded_sha)) = recorded.get(&name)
            && *recorded_sha != actual
        {
            drifted.push(format!("{name}: recorded {recorded_sha}, on disk {actual}"));
        }
    }

    assert!(
        drifted.is_empty(),
        "these fixtures have changed since the oracle last saw them, so the recorded verdicts \
         describe documents that no longer exist. Re-run \
         `cargo run --manifest-path tools/oracle/Cargo.toml`.\n  {}",
        drifted.join("\n  ")
    );
}

#[test]
fn both_validators_read_the_same_schema_bytes() {
    let recording = recording();

    let oracle_schema = recording["oracle"]["schema_it_reads_sha256"]
        .as_str()
        .expect("the recording says which schema the oracle read");
    let ours = recording["vendored_schema"]["sha256"]
        .as_str()
        .expect("the recording says which schema this repository vendors");

    assert_eq!(
        oracle_schema, ours,
        "the oracle validated against different bytes from the ones this crate validates \
         against, so any disagreement below would be about inputs rather than about rules"
    );

    // And those bytes are still the ones on disk here.
    let vendored = support::workspace_root().join(
        recording["vendored_schema"]["path"]
            .as_str()
            .expect("the recording names the vendored schema"),
    );
    let bytes =
        fs::read(&vendored).unwrap_or_else(|e| panic!("cannot read {}: {e}", vendored.display()));
    assert_eq!(
        sha256_hex(&bytes),
        ours,
        "the vendored schema has changed since the oracle was recorded against it"
    );
}

#[test]
fn every_verdict_agrees_except_the_recorded_divergences() {
    let recording = recording();
    let recorded = recorded_verdicts(&recording);
    let validator = support::validator();

    let divergences: BTreeMap<&str, &str> = KNOWN_DIVERGENCES.iter().copied().collect();

    let mut unexpected = Vec::new();
    let mut converged = Vec::new();

    for path in support::fixtures() {
        let name = path
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let (oracle_fails, _) = recorded[&name].clone();

        let document = support::fixture(&name);
        let report = validator.validate(&document);
        let we_fail = report.should_gate(conform_core::GatePolicy::default());

        let agree = we_fail == oracle_fails;
        let expected_to_diverge = divergences.contains_key(name.as_str());

        if agree && expected_to_diverge {
            converged.push(name.clone());
        } else if !agree && !expected_to_diverge {
            unexpected.push(format!(
                "{name}: oracle {}, this crate {}\n      our errors: {:?}",
                verdict(oracle_fails),
                verdict(we_fail),
                support::errors(&report)
            ));
        }
    }

    assert!(
        unexpected.is_empty(),
        "this crate and the validator it replaces disagree on documents where they were expected \
         to agree. Do NOT resolve this by editing either side to match: one of them is wrong, and \
         which one is the finding. Record it in KNOWN_DIVERGENCES with the reason, or fix the \
         defect.\n  {}",
        unexpected.join("\n  ")
    );

    assert!(
        converged.is_empty(),
        "these fixtures are listed in KNOWN_DIVERGENCES but the two validators now agree on them. \
         That is good news and it means the list is out of date — remove the entry, so the list \
         stays a record of live disagreements rather than a standing exemption.\n  {converged:?}"
    );
}

/// The negative control for the test above.
///
/// `every_verdict_agrees_except_the_recorded_divergences` would pass just as
/// happily over a corpus where the oracle failed everything and so did we —
/// agreement is cheap if nothing discriminates. This asserts the corpus
/// actually contains both outcomes on both sides, so agreement means
/// something.
#[test]
fn the_corpus_discriminates() {
    let recording = recording();
    let recorded = recorded_verdicts(&recording);
    let validator = support::validator();

    let oracle_passes = recorded.values().filter(|(fails, _)| !fails).count();
    let oracle_fails = recorded.values().filter(|(fails, _)| *fails).count();
    assert!(
        oracle_passes >= 2 && oracle_fails >= 2,
        "the oracle passes {oracle_passes} and fails {oracle_fails} of the corpus; a corpus that \
         lands almost entirely on one side cannot show that two validators agree"
    );

    let ours: Vec<bool> = support::fixtures()
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .expect("a file has a name")
                .to_string_lossy()
                .into_owned();
            support::fails(&validator, &support::fixture(&name))
        })
        .collect();
    assert!(
        ours.iter().filter(|f| **f).count() >= 2 && ours.iter().filter(|f| !**f).count() >= 2,
        "this crate does not both pass and fail enough of the corpus for agreement to mean \
         anything: {ours:?}"
    );
}

fn verdict(fails: bool) -> &'static str {
    if fails { "FAIL" } else { "PASS" }
}
