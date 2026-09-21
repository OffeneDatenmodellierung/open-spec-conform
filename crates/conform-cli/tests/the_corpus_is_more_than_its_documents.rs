//! Two facts the specifications state, that no single document can hold.
//!
//! Every validator in this family checks one document. That is what makes a
//! verdict reproducible — one file, one vendored schema, one answer — and it
//! is also why two things upstream says outright went unchecked here until
//! now:
//!
//! - **an ODPS `contractId` names an ODCS contract.** `$defs/InputPort`
//!   *requires* one. The published schema types it `"type": "string"` and
//!   stops, because the thing that answers to it is in another file. The link
//!   between the two standards is the whole reason an estate publishes both.
//! - **an ODCS `id` is "a unique identifier used to reduce the risk of dataset
//!   name collisions"** — between contracts, which one contract cannot see.
//!
//! This file is about the pass that can see them, and about the one mistake it
//! must never make.
//!
//! # The mistake
//!
//! `conform validate products/` over a directory holding only ODPS documents
//! loads **no contract at all**. A two-valued resolver answers "not found" and
//! reports every `contractId` in that directory as dangling — a screenful of
//! false alarms on a run where nothing is wrong and nothing was even looked
//! at. A gate that does that gets switched off, and then it protects nothing.
//!
//! So the resolver returns `conform_core::Resolution`, whose three outcomes
//! keep "checked, and genuinely absent" apart from "nobody looked". The test
//! [`products_alone_are_never_reported_as_dangling`] is the one that matters
//! most in this file.

mod support;

use support::{conform, conform_json, scratch, write};

/// A contract, by `id`, with everything else exemplary so the only findings
/// are the ones a test asked for.
fn contract(id: &str, name: &str) -> String {
    format!(
        "version: 1.0.0\n\
         apiVersion: v3.1.0\n\
         kind: DataContract\n\
         id: {id}\n\
         name: {name}\n\
         status: active\n\
         description:\n  \
           purpose: A contract for a corpus test.\n\
         servers:\n  \
           - server: orders-prod\n    \
             type: postgresql\n    \
             host: db.example.com\n    \
             port: 5432\n    \
             database: retail\n    \
             schema: public\n\
         schema:\n  \
           - name: orders\n    \
             logicalType: object\n    \
             physicalType: table\n\
         team:\n  \
           name: Retail Data Platform\n"
    )
}

/// A product whose three contract references — the three places the vendored
/// ODPS schema puts one — point wherever the caller says.
fn product(input: &str, output: &str, nested: &str) -> String {
    format!(
        "apiVersion: v1.0.0\n\
         kind: DataProduct\n\
         id: order-analytics\n\
         name: Order Analytics\n\
         version: 1.4.0\n\
         status: active\n\
         description:\n  \
           purpose: Publish curated order facts.\n\
         inputPorts:\n  \
           - name: raw-orders\n    \
             version: 2.3.0\n    \
             contractId: {input}\n\
         outputPorts:\n  \
           - name: orders_gold\n    \
             type: tables\n    \
             version: 1.4.0\n    \
             contractId: {output}\n    \
             inputContracts:\n      \
               - id: {nested}\n        \
                 version: 2.3.0\n\
         team:\n  \
           name: Retail Data Platform\n"
    )
}

/// Every diagnostic in a run, as `(code, pointer, message)`.
///
/// Read from the `--json` envelope rather than scraped off the human
/// rendering: the terminal output is escaped at the boundary, so asserting on
/// it would be asserting on the escaper as much as on the rule.
fn diagnostics(args: &[&str]) -> Vec<(String, String, String)> {
    let (envelope, _) = conform_json(args);
    envelope["diagnostics"]
        .as_array()
        .expect("the envelope carries a diagnostics array")
        .iter()
        .map(|diagnostic| {
            (
                diagnostic["code"].as_str().unwrap_or_default().to_owned(),
                diagnostic["location"]["pointer"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                diagnostic["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect()
}

/// Every diagnostic raised under one code, as `(pointer, message)`.
fn under(args: &[&str], code: &str) -> Vec<(String, String)> {
    diagnostics(args)
        .into_iter()
        .filter(|(raised, _, _)| raised == code)
        .map(|(_, pointer, message)| (pointer, message))
        .collect()
}

// ---------------------------------------------------------------------------
// Resolution: the three outcomes, each shown happening.
// ---------------------------------------------------------------------------

#[test]
fn a_contract_in_the_run_resolves_the_reference() {
    let directory = scratch("corpus-resolved");
    write(&directory, "orders.yaml", &contract("orders-v1", "Orders"));
    let path = write(
        &directory,
        "product.yaml",
        &product("orders-v1", "orders-v1", "orders-v1"),
    );
    let arguments = ["validate", directory.to_str().expect("a UTF-8 path")];

    let resolved = under(&arguments, "CLI202");
    let pointers: Vec<&str> = resolved
        .iter()
        .map(|(pointer, _)| pointer.as_str())
        .collect();

    assert_eq!(
        pointers,
        [
            "/inputPorts/0/contractId",
            "/outputPorts/0/contractId",
            "/outputPorts/0/inputContracts/0/id",
        ],
        "the three places the ODPS schema puts a contract reference were not all followed"
    );
    for (_, message) in &resolved {
        assert!(
            message.contains("orders.yaml"),
            "a resolved reference does not say which contract answered it: {message}"
        );
    }

    assert!(
        under(&arguments, "CLI200").is_empty(),
        "a reference that resolves was also reported as dangling"
    );
    assert!(
        path.ends_with("product.yaml"),
        "the product was not written where this test believes"
    );
}

#[test]
fn a_reference_to_a_contract_that_is_not_here_is_a_warning() {
    let directory = scratch("corpus-dangling");
    write(&directory, "orders.yaml", &contract("orders-v1", "Orders"));
    write(
        &directory,
        "product.yaml",
        &product("orders-v1", "orders-gold-v1", "orders-v1"),
    );
    let arguments = ["validate", directory.to_str().expect("a UTF-8 path")];

    let dangling = under(&arguments, "CLI200");
    assert_eq!(
        dangling.len(),
        1,
        "expected exactly the one reference that names nothing: {dangling:?}"
    );
    assert_eq!(dangling[0].0, "/outputPorts/0/contractId");
    assert!(
        dangling[0].1.contains("orders-gold-v1"),
        "the finding does not quote the reference it is about: {}",
        dangling[0].1
    );

    // The claim it makes is narrow and the message has to say so: this run
    // searched what it loaded, and that is all.
    assert!(
        dangling[0].1.contains("this run loaded"),
        "the finding claims more than it checked: {}",
        dangling[0].1
    );
}

/// **The one that matters.** A directory of products alone is a run in which
/// nobody looked, and it must not read as a run in which everything is broken.
#[test]
fn products_alone_are_never_reported_as_dangling() {
    let directory = scratch("corpus-products-only");
    write(
        &directory,
        "product.yaml",
        &product("orders-v1", "orders-gold-v1", "orders-v1"),
    );
    let arguments = ["validate", directory.to_str().expect("a UTF-8 path")];

    assert!(
        under(&arguments, "CLI200").is_empty(),
        "a run that loaded no contract reported a reference as dangling — this is the exact \
         false alarm `conform_core::Resolution` exists to prevent, and it is worse than not \
         having the rule at all"
    );

    let not_inspected = under(&arguments, "CLI201");
    assert_eq!(
        not_inspected.len(),
        3,
        "every reference in the document should be recorded as not followed: {not_inspected:?}"
    );
    for (_, message) in &not_inspected {
        assert!(
            message.contains("outside-document-set"),
            "the reason nobody looked is not on the record: {message}"
        );
    }

    // Not silence, either. "I followed this and it was fine" and "I never
    // followed this" are different facts, and a run that printed nothing would
    // be claiming the first while doing the second.
    assert!(
        !not_inspected.is_empty(),
        "an unfollowed reference was passed over in silence"
    );
}

/// The second route into "nobody looked", and it is not a corner case: a run
/// holding a contract it could not read has an incomplete corpus, and the
/// contract being looked for may be exactly the one that would not read.
#[test]
fn an_unreadable_contract_makes_absence_unclaimable() {
    let directory = scratch("corpus-incomplete");
    // Discovered as ODCS — it says `kind: DataContract` — and carrying no
    // `id`, which its own adapter reports as the schema violation it is.
    write(
        &directory,
        "broken.yaml",
        "apiVersion: v3.1.0\nkind: DataContract\nstatus: active\n",
    );
    write(
        &directory,
        "product.yaml",
        &product("orders-v1", "orders-v1", "orders-v1"),
    );
    let arguments = ["validate", directory.to_str().expect("a UTF-8 path")];

    assert!(
        under(&arguments, "CLI200").is_empty(),
        "absence was claimed over a corpus this run could not fully read"
    );
    let not_inspected = under(&arguments, "CLI201");
    assert_eq!(not_inspected.len(), 3);
    for (_, message) in &not_inspected {
        assert!(
            message.contains("inspection-failed"),
            "the reason is not the one that applies here: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Collision: an `id` the schema calls unique and cannot check.
// ---------------------------------------------------------------------------

#[test]
fn two_contracts_sharing_an_id_are_both_reported_and_each_names_the_other() {
    let directory = scratch("corpus-collision");
    write(
        &directory,
        "orders-a.yaml",
        &contract("orders-v1", "Finance"),
    );
    write(&directory, "orders-b.yaml", &contract("orders-v1", "Sales"));
    let arguments = ["validate", directory.to_str().expect("a UTF-8 path")];

    let (envelope, _) = conform_json(&arguments);
    let collisions: Vec<(&str, &str)> = envelope["diagnostics"]
        .as_array()
        .expect("the envelope carries a diagnostics array")
        .iter()
        .filter(|diagnostic| diagnostic["code"] == "CLI203")
        .map(|diagnostic| {
            (
                diagnostic["location"]["document"]
                    .as_str()
                    .unwrap_or_default(),
                diagnostic["message"].as_str().unwrap_or_default(),
            )
        })
        .collect();

    assert_eq!(
        collisions.len(),
        2,
        "a collision is a fact about two documents and has to be reported on both — either may \
         be the one that should change: {collisions:?}"
    );

    let (first_document, first_message) = collisions[0];
    let (second_document, second_message) = collisions[1];
    assert!(first_document.ends_with("orders-a.yaml"));
    assert!(second_document.ends_with("orders-b.yaml"));
    assert!(
        first_message.contains("orders-b.yaml"),
        "the finding on the first contract does not name the other: {first_message}"
    );
    assert!(
        second_message.contains("orders-a.yaml"),
        "the finding on the second contract does not name the other: {second_message}"
    );
}

/// The control. Distinct identifiers are not a collision, and neither is one
/// contract on its own.
#[test]
fn distinct_identifiers_are_not_a_collision() {
    let directory = scratch("corpus-no-collision");
    write(
        &directory,
        "orders-a.yaml",
        &contract("orders-v1", "Finance"),
    );
    write(&directory, "orders-b.yaml", &contract("orders-v2", "Sales"));
    let arguments = ["validate", directory.to_str().expect("a UTF-8 path")];

    assert!(
        under(&arguments, "CLI203").is_empty(),
        "two contracts with different identifiers were reported as colliding"
    );
}

/// One file named twice on the command line is a fact about the invocation,
/// not about the documents. Reporting it would be a false alarm of the most
/// annoying kind: one nobody can fix by editing anything.
#[test]
fn one_contract_named_twice_is_not_a_collision() {
    let directory = scratch("corpus-named-twice");
    let path = write(&directory, "orders.yaml", &contract("orders-v1", "Orders"));
    let arguments = ["validate", path.as_str(), path.as_str()];

    assert!(
        under(&arguments, "CLI203").is_empty(),
        "the same path given twice was reported as two contracts colliding"
    );
}

// ---------------------------------------------------------------------------
// And none of it moves a verdict.
// ---------------------------------------------------------------------------

/// The property the whole family is built on. The adapters' errors are exactly
/// their published schemas' findings; a corpus rule that gated would undo that
/// from the outside.
#[test]
fn nothing_the_corpus_pass_raises_is_an_error() {
    let directory = scratch("corpus-verdict");
    write(
        &directory,
        "orders-a.yaml",
        &contract("orders-v1", "Finance"),
    );
    write(&directory, "orders-b.yaml", &contract("orders-v1", "Sales"));
    write(
        &directory,
        "product.yaml",
        &product("orders-v1", "nothing-here", "orders-v1"),
    );
    let path = directory.to_str().expect("a UTF-8 path");

    // The run really does hold findings from all four corpus codes, so the
    // assertions below are about a run where this pass had plenty to say.
    let raised: Vec<String> = diagnostics(&["validate", path])
        .into_iter()
        .map(|(code, _, _)| code)
        .collect();
    for code in ["CLI200", "CLI201", "CLI202", "CLI203"] {
        let expected = code != "CLI201";
        assert_eq!(
            raised.iter().any(|found| found == code),
            expected,
            "this run was built to exercise the corpus pass and {code} did not behave as the \
             fixture intends: {raised:?}"
        );
    }

    let (envelope, exit) = conform_json(&["validate", path]);
    assert_eq!(
        envelope["summary"]["error"], 0,
        "the corpus pass raised an error, which would change a verdict the schema gave"
    );
    assert_eq!(exit, 0);

    // And under the strictest ordinary gate — errors — this run still passes,
    // which is the same statement made where a user meets it.
    let gated = conform(&["validate", path, "--gate", "errors"]);
    assert_eq!(
        gated.code, 0,
        "a run whose only findings are corpus warnings failed an errors gate"
    );
}
