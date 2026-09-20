//! The vendored schema's per-technology server dispatch never runs, and this
//! proves the mechanism rather than asserting it.
//!
//! `servers`' value schema is:
//!
//! ```json
//! { "$ref": "#/$defs/BaseServer", "allOf": [ … 19 if/then branches … ] }
//! ```
//!
//! and the document declares `"$schema": "http://json-schema.org/draft-07/schema#"`.
//! Under draft-07 a `$ref` alongside other keywords means **every sibling
//! keyword is ignored** — the rule that changed in 2019-09, where `$ref`
//! became an ordinary applicator. So `BaseServer` applies and the entire
//! dispatch does not, and a `type: postgres` server that satisfies nothing
//! `PostgresServer` requires conforms.
//!
//! That is a claim about somebody else's schema, and a claim about somebody
//! else's schema deserves better evidence than a comment. The first test below
//! isolates it: the same `allOf` fires with the `$ref` removed, and fires
//! again with the `$ref` left in but the draft moved forward. Two controls,
//! one variable.
//!
//! # What this crate does about it
//!
//! Reports it, and does not route around it. The verdict stays the schema's
//! verdict — so this crate agrees with `validate_odcl_internal` on these
//! documents, as `tests/oracle_agreement.rs` shows — and every affected server
//! gets a [`codes::SERVER_TYPE_NOT_INSPECTED`] note saying which check did not
//! happen. Reimplementing nineteen server sub-schemas to work around an
//! upstream defect would make this crate the only tool in the estate that
//! rejects these documents, which is how a validator stops being used.
//!
//! Recorded upstream as F-005 in `docs/findings/0001-upstream-defects.md`.

mod support;

use conform_core::{SpecRef, Validator};
use conform_lexicon::{LexiconValidator, codes};
use serde_json::Value;

/// A schema with the same shape as the real one's `servers` entry: a `$ref`
/// with an `allOf` sibling that would require `host` when `t` is `p`.
fn probe_schema(draft: &str, with_ref_sibling: bool) -> String {
    let dispatch = r#"[{"if":{"properties":{"t":{"const":"p"}},"required":["t"]},"then":{"required":["host"]}}]"#;
    let server = if with_ref_sibling {
        format!(r##"{{"$ref":"#/$defs/BaseServer","allOf":{dispatch}}}"##)
    } else {
        format!(r#"{{"allOf":{dispatch}}}"#)
    };
    format!(
        r#"{{"$schema":"{draft}","type":"object","properties":{{"s":{server}}},"$defs":{{"BaseServer":{{"type":"object"}}}}}}"#
    )
}

fn probe(draft: &str, with_ref_sibling: bool) -> Vec<String> {
    let validator = LexiconValidator::from_schema_str(
        &probe_schema(draft, with_ref_sibling),
        SpecRef::new("probe"),
        "a probe schema written by tests/the_server_dispatch_is_dead.rs",
        "probe",
    )
    .expect("the probe schema compiles");
    support::errors(&validator.validate_text("probe", "s:\n  t: p\n"))
}

const DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";
const DRAFT_2019_09: &str = "https://json-schema.org/draft/2019-09/schema";

#[test]
fn a_ref_sibling_suppresses_its_allof_under_draft_07_and_not_after() {
    // The variable: the `$ref` sibling, under the draft the ODCL schema
    // declares. The dispatch does not run.
    assert!(
        probe(DRAFT_07, true).is_empty(),
        "a `$ref` sibling no longer suppresses its `allOf` under draft-07 — the mechanism behind \
         ODCL304 has changed, so re-check the finding before trusting it: {:?}",
        probe(DRAFT_07, true)
    );

    // Control one: same draft, `$ref` removed. The dispatch runs.
    assert!(
        !probe(DRAFT_07, false).is_empty(),
        "the probe's `allOf` does not fire even with the `$ref` removed, so the probe proves \
         nothing about `$ref`"
    );

    // Control two: same `$ref` sibling, draft moved forward. The dispatch runs.
    assert!(
        !probe(DRAFT_2019_09, true).is_empty(),
        "the probe's `allOf` does not fire under 2019-09 either, so the draft is not the variable"
    );
}

#[test]
fn the_vendored_schema_really_has_that_shape() {
    let path = support::workspace_root().join("schemas/odcl-json-schema-1.2.1.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let schema: Value = serde_json::from_str(&text).expect("the vendored schema is JSON");

    assert_eq!(
        schema["$schema"].as_str(),
        Some(DRAFT_07),
        "the vendored schema no longer declares draft-07, so the dispatch may have come alive — \
         re-check ODCL304 rather than assuming"
    );

    let servers = schema["properties"]["servers"]["additionalProperties"]
        .as_object()
        .expect("`servers` has a value schema");
    assert!(
        servers.contains_key("$ref"),
        "the `$ref` has been lifted out of the `servers` value schema, so the dispatch now works"
    );
    assert!(
        servers.contains_key("allOf"),
        "the `servers` dispatch has gone; ODCL304 now describes nothing"
    );

    let branches = servers["allOf"]
        .as_array()
        .expect("the dispatch is an array of branches");
    assert_eq!(
        branches.len(),
        19,
        "the vendored schema carries {} dispatch branches, not the 19 this finding was written \
         against",
        branches.len()
    );
}

#[test]
fn a_server_that_satisfies_nothing_its_type_requires_still_conforms() {
    let validator = support::validator();
    let report = validator.validate(&support::fixture("conformant-server-dispatch-is-dead.yaml"));

    // `PostgresServer` requires host, port, database and schema, and types all
    // four. The fixture's `staging` server gets every one of them wrong and
    // `production` omits them entirely. The published schema objects to none
    // of it, and neither does this crate: the verdict is the schema's.
    assert!(
        support::errors(&report).is_empty(),
        "the dispatch has started working and this fixture now fails — which is good news about \
         upstream, and means ODCL304 and this test need revisiting: {:?}",
        support::errors(&report)
    );

    // What this crate adds: one note per affected server, naming the check
    // that did not happen. Three servers, three notes — a rule that reported
    // once per document would look identical on a one-server fixture.
    assert_eq!(
        support::count_code(&report, codes::SERVER_TYPE_NOT_INSPECTED),
        3,
        "expected a note for each of `production`, `staging` and `archive`: {:?}",
        support::codes_in(&report)
    );

    let note = report
        .iter()
        .find(|d| d.code.as_str() == codes::SERVER_TYPE_NOT_INSPECTED)
        .expect("the note is there");
    assert_eq!(
        note.severity,
        conform_core::Severity::Info,
        "a note about this crate's own coverage must not be a complaint about the document"
    );
    assert!(
        note.help.is_some(),
        "a reader told a check did not happen needs to be told why"
    );
}

/// The control for the rule, so it cannot fire on everything.
///
/// A server with no `type`, and a `type` the dispatch has no branch for, are
/// both "nothing was skipped" rather than findings.
#[test]
fn the_note_fires_only_where_a_branch_was_actually_skipped() {
    let validator = support::validator();

    let untyped = validator.validate_text(
        "untyped",
        "dataContractSpecification: 1.2.1\nid: x\ninfo:\n  title: T\n  version: '1'\nservers:\n  s:\n    description: no type at all\n",
    );
    assert_eq!(
        support::count_code(&untyped, codes::SERVER_TYPE_NOT_INSPECTED),
        0,
        "a server declaring no type had no branch to skip: {:?}",
        support::codes_in(&untyped)
    );

    let unknown_type = validator.validate_text(
        "unknown-type",
        "dataContractSpecification: 1.2.1\nid: x\ninfo:\n  title: T\n  version: '1'\nservers:\n  s:\n    type: some-technology-with-no-branch\n",
    );
    assert_eq!(
        support::count_code(&unknown_type, codes::SERVER_TYPE_NOT_INSPECTED),
        0,
        "a type the schema has no sub-schema for had nothing to skip: {:?}",
        support::codes_in(&unknown_type)
    );

    // And a type that does have one still gets the note, or the two controls
    // above would be satisfied by a rule that never fires.
    let dispatched = validator.validate_text(
        "dispatched",
        "dataContractSpecification: 1.2.1\nid: x\ninfo:\n  title: T\n  version: '1'\nservers:\n  s:\n    type: postgres\n",
    );
    assert_eq!(
        support::count_code(&dispatched, codes::SERVER_TYPE_NOT_INSPECTED),
        1
    );
}
