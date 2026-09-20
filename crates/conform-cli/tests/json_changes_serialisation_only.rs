//! `--json` changes the spelling, and nothing else.
//!
//! This is the test that stops the two renderers drifting apart, and it is
//! worth being precise about what it holds them to. For the *same* inputs, the
//! human-readable run and the `--json` run must produce:
//!
//! - the same **set of diagnostics** — same codes, same severities, same
//!   locations, in the same order and with the same multiplicity;
//! - the same **exit code**.
//!
//! Anything else is a bug in one of them, and without this test it is a bug
//! nobody would notice until a dashboard and a terminal disagreed about
//! whether a contract was conformant.
//!
//! # Why messages are compared by position and not by text
//!
//! They are deliberately *not* byte-identical across the two sinks, and that
//! is the one intended difference. A message quotes the document verbatim, so
//! a hostile document can carry an ANSI sequence or a bidirectional override
//! into it. The terminal renderings neutralise that; `--json` must not,
//! because JSON string encoding is already the right escaping for that sink
//! and applying both corrupts the data. `tests/hostile_text_is_neutralised.rs`
//! holds both halves of that. Here the corpus is ordinary text, where the
//! escaping is the identity function — and this test asserts that too, so the
//! exemption cannot quietly widen.

use std::fmt::Write as _;

mod support;

use support::{conform, conform_json, in_workspace};

/// Every invocation this test holds the two renderers to.
///
/// Deliberately wide: both single-document adapters over their whole fixture
/// corpora, a bundle, both registry commands, all three gate policies, and two
/// ways of failing before any document is read. A pair that agreed only on
/// clean input would prove very little.
fn invocations() -> Vec<Vec<String>> {
    let contracts = in_workspace("crates/conform-odcs/tests/fixtures");
    let products = in_workspace("crates/conform-odps/tests/fixtures");
    let bundle = in_workspace("crates/conform-okf/tests/fixtures/okf-upstream/acme_retail");

    let mut invocations = Vec::new();
    for gate in [
        vec![],
        vec!["--gate", "never"],
        vec!["--gate", "errors"],
        vec!["--gate", "warnings"],
        vec!["--check"],
    ] {
        for base in [
            vec!["validate", contracts.as_str()],
            vec!["validate", products.as_str()],
            vec!["validate", bundle.as_str()],
            vec!["validate", contracts.as_str(), "--spec", "odps"],
            vec![
                "validate",
                contracts.as_str(),
                products.as_str(),
                bundle.as_str(),
            ],
            vec!["registry", "list"],
            vec!["registry", "verify"],
            vec!["registry", "list", "--spec", "okf"],
            // Two runs that cannot happen at all. They must agree on exit 2
            // just as firmly as the runs that can.
            vec!["validate", contracts.as_str(), "--spec", "nonesuch"],
            vec!["validate", contracts.as_str(), "--spec", "cads"],
        ] {
            let mut argv: Vec<String> = base.iter().map(|a| (*a).to_owned()).collect();
            argv.extend(gate.iter().map(|a| (*a).to_owned()));
            invocations.push(argv);
        }
    }
    invocations
}

/// One diagnostic, reduced to the three things both renderings must agree on.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    severity: String,
    code: String,
    location: String,
}

#[test]
fn the_same_inputs_produce_the_same_findings_and_the_same_exit_code() {
    let mut checked = 0usize;

    for argv in invocations() {
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();

        let human = conform(&args);
        let (json, json_code) = conform_json(&args);

        assert_eq!(
            human.code,
            json_code,
            "exit codes differ for `conform {}`",
            args.join(" ")
        );
        assert_eq!(
            i64::from(human.code),
            json["exit_code"].as_i64().expect("exit_code is a number"),
            "the envelope's own `exit_code` disagrees with the process for `conform {}`",
            args.join(" ")
        );

        let from_human = parse_human(&human.stdout);
        let from_json = parse_json(&json);
        assert_eq!(
            from_human,
            from_json,
            "the two renderings disagree about what was found for `conform {}`",
            args.join(" ")
        );

        checked += from_human.len();
    }

    // A guard against the test passing because both renderers emitted nothing.
    assert!(
        checked > 500,
        "only {checked} diagnostics were compared across {} invocations; the corpus has \
         collapsed and this test is no longer proving anything",
        invocations().len()
    );
}

#[test]
fn the_corpus_carries_nothing_the_terminal_escaping_would_alter() {
    // The premise of comparing locations as text: over this corpus,
    // `escape::for_terminal` is the identity, so a difference between the two
    // renderings above can only be a real disagreement and never the one
    // intended difference between the sinks.
    let contracts = in_workspace("crates/conform-odcs/tests/fixtures");
    let bundle = in_workspace("crates/conform-okf/tests/fixtures/okf-upstream/acme_retail");
    let (json, _) = conform_json(&["validate", &contracts, &bundle]);

    let mut seen = 0usize;
    for diagnostic in json["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
    {
        for field in ["message", "help"] {
            if let Some(text) = diagnostic[field].as_str() {
                assert_eq!(
                    conform_cli::escape::for_terminal(text),
                    text,
                    "fixture text would be rewritten by the terminal escaping: {text:?}"
                );
                seen += 1;
            }
        }
    }
    // A floor, not a target: it only has to be high enough that an empty
    // corpus cannot pass. The run above yields 98 strings today.
    assert!(seen > 50, "only {seen} strings were checked");
}

/// Pull the diagnostics out of the human rendering.
///
/// The format is the one `human`'s documentation specifies: a header line
/// indented by exactly two spaces, everything belonging to it indented by six.
/// Nothing else in the output starts with two spaces and a severity word.
fn parse_human(text: &str) -> Vec<Finding> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("  ")?;
            // Six-space continuations, and the spec-pane lines, are not
            // findings.
            if rest.starts_with(' ') {
                return None;
            }
            let (severity, rest) = rest.split_once(' ')?;
            if !matches!(severity, "error" | "warning" | "info") {
                return None;
            }
            let (code, location) = rest.split_once(" at ")?;
            Some(Finding {
                severity: severity.to_owned(),
                code: code.to_owned(),
                location: location.to_owned(),
            })
        })
        .collect()
}

/// Pull the same three things out of the envelope.
///
/// The location is reassembled here from its parts rather than read from a
/// pre-rendered string, deliberately: the point is that the *structured*
/// location and the *printed* location describe the same place, which a shared
/// helper would assume rather than test.
fn parse_json(envelope: &serde_json::Value) -> Vec<Finding> {
    envelope["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .map(|diagnostic| {
            let at = &diagnostic["location"];
            let mut location = at["document"]
                .as_str()
                .expect("every location names a document")
                .to_owned();
            if let Some(line) = at["line"].as_u64() {
                let _ = write!(location, ":{line}");
                if let Some(column) = at["column"].as_u64() {
                    let _ = write!(location, ":{column}");
                }
            }
            if let Some(pointer) = at["pointer"].as_str() {
                let _ = write!(location, " ({pointer})");
            }
            Finding {
                severity: diagnostic["severity"]
                    .as_str()
                    .expect("severity is a string")
                    .to_owned(),
                code: diagnostic["code"]
                    .as_str()
                    .expect("code is a string")
                    .to_owned(),
                location,
            }
        })
        .collect()
}
