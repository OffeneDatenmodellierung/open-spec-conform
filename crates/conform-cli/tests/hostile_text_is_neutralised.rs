//! A hostile document cannot rewrite the operator's terminal — and cannot be
//! corrupted on its way into `--json` either.
//!
//! # The obligation this test discharges
//!
//! `conform-okf` states it explicitly, and every adapter in this family
//! inherits it: diagnostic messages quote third-party content **verbatim** and
//! are deliberately *not* escaped, because escaping is not idempotent — a
//! backslash doubles on every pass — so it has to happen exactly once, at the
//! point of display. This crate is that point.
//!
//! So there are two obligations, and they point in opposite directions:
//!
//! | Sink | Must | Because |
//! |---|---|---|
//! | terminal | neutralise | a `ESC` sequence recolours or retitles the operator's screen, and a U+202E reorders a line so it reads as the opposite of what it says |
//! | `--json` | preserve | JSON string encoding is already the correct escaping there, and a consumer must receive the bytes the document really held |
//!
//! Testing only one of them is worse than testing neither, because either one
//! alone is trivially satisfied by a renderer that has got the other wrong.
//!
//! # The vector is real, not simulated
//!
//! The hostile text below travels the whole ordinary path: a YAML document on
//! disk, parsed by `conform-odcs`, through the hygiene rule at
//! `conform-odcs/src/rules.rs` that reports `` `status` is `{status}` `` —
//! which interpolates a document-controlled scalar straight into a message,
//! exactly as the design says it should. Nothing here injects a diagnostic by
//! hand; if that rule stopped quoting the document, this test would stop
//! finding its payload and would say so.
//!
//! It is held for `conform-lexicon` too, through that crate's `ODCL205`
//! (`` `info.status` is `{status}` ``), and for the same reason: every adapter
//! in this family hands the escaping obligation here, so every adapter this
//! binary drives has to be shown discharging it. An adapter wired in without
//! its half of this test is an adapter whose payloads reach the terminal.

use std::fmt::Write as _;

mod support;

use support::{conform, conform_json, scratch, write};

/// U+202E RIGHT-TO-LEFT OVERRIDE, an ANSI colour sequence, an OSC window-title
/// sequence terminated by BEL, and a zero-width space.
///
/// Written as escapes rather than as literal bytes so that reading this file
/// is not itself an exercise in trusting your terminal.
const HOSTILE: &str = "act\u{202e}evi\u{1b}[31mtcani\u{1b}]0;pwned\u{7}ve\u{200b}d";

/// A minimal ODCS contract whose `status` is the payload.
fn hostile_contract() -> String {
    format!(
        "version: 1.0.0\n\
         apiVersion: v3.1.0\n\
         kind: DataContract\n\
         id: 53581432-6c55-4ba2-a65f-72344a91553a\n\
         status: {}\n",
        yaml_quoted(HOSTILE)
    )
}

/// The payload as a YAML double-quoted scalar.
///
/// Written through YAML's own `\uXXXX` escapes rather than as raw bytes,
/// because YAML forbids most control characters in a scalar and a parser that
/// rejected the fixture would make this test pass for the wrong reason. What
/// reaches the adapter is the real character either way.
fn yaml_quoted(text: &str) -> String {
    let mut quoted = String::from("\"");
    for character in text.chars() {
        if character.is_ascii_graphic() && character != '"' && character != '\\' {
            quoted.push(character);
        } else {
            let _ = write!(quoted, "\\u{:04X}", character as u32);
        }
    }
    quoted.push('"');
    quoted
}

/// A minimal ODCL document whose `info.status` is the payload.
///
/// ODCL carries no `kind`; the root `dataContractSpecification` is what routes
/// it, and `id` and `info.title`/`info.version` are what the schema requires.
/// The status itself is published as `examples` rather than an `enum`, so the
/// schema has nothing to say about it and the finding that quotes it is the
/// hygiene rule's — exactly the shape of the ODCS vector above.
fn hostile_lexicon() -> String {
    format!(
        "dataContractSpecification: 1.2.1\n\
         id: urn:datacontract:checkout:orders\n\
         info:\n\
        \u{20}\u{20}title: Orders\n\
        \u{20}\u{20}version: 1.0.0\n\
        \u{20}\u{20}status: {}\n",
        yaml_quoted(HOSTILE)
    )
}

/// Everything this test insists must never reach a terminal.
const FORBIDDEN: [char; 4] = ['\u{202e}', '\u{1b}', '\u{7}', '\u{200b}'];

#[test]
fn the_payload_really_does_reach_a_diagnostic() {
    // The negative control for the two tests below. If the adapter stopped
    // quoting the document, they would both pass while proving nothing.
    let directory = scratch("hostile-control");
    let path = write(&directory, "orders.yaml", &hostile_contract());
    let (json, _) = conform_json(&["validate", &path]);

    let carrying = json["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .filter(|d| d["message"].as_str().is_some_and(|m| m.contains(HOSTILE)))
        .count();

    assert_eq!(
        carrying, 1,
        "expected exactly one diagnostic quoting the document's `status`; \
         the adapter's hygiene rule may have stopped interpolating it"
    );
}

#[test]
fn no_control_or_bidi_character_survives_into_the_terminal_rendering() {
    let directory = scratch("hostile-human");
    let path = write(&directory, "orders.yaml", &hostile_contract());
    let output = conform(&["validate", &path]);

    for character in FORBIDDEN {
        assert!(
            !output.stdout.contains(character),
            "U+{:04X} reached stdout:\n{}",
            character as u32,
            output.stdout.escape_default()
        );
    }

    // Neutralised, not deleted. An operator who is shown a *different* string
    // from the one the document held cannot act on the finding, and would have
    // no way to know a character had been removed.
    for expected in ["<U+202E>", "<U+001B>", "<U+0007>", "<U+200B>"] {
        assert!(
            output.stdout.contains(expected),
            "{expected} is missing, so the payload was dropped rather than shown"
        );
    }

    // And the surrounding text is untouched: escaping is surgical.
    assert!(output.stdout.contains("act<U+202E>evi<U+001B>[31mtcani"));
}

#[test]
fn json_receives_the_bytes_the_document_really_held() {
    let directory = scratch("hostile-json");
    let path = write(&directory, "orders.yaml", &hostile_contract());
    let (json, _) = conform_json(&["validate", &path]);

    let message = json["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .find_map(|d| d["message"].as_str().filter(|m| m.contains("status")))
        .expect("the status rule reported something");

    assert!(
        message.contains(HOSTILE),
        "the payload was altered on its way into JSON: {}",
        message.escape_default()
    );

    // The failure mode this guards against specifically: escaping twice. A
    // consumer reading `<U+202E>` where the document held one character has
    // been handed corrupted data, and cannot recover the original.
    for terminal_escape in ["<U+202E>", "<U+001B>", "<U+0007>", "<U+200B>"] {
        assert!(
            !message.contains(terminal_escape),
            "{terminal_escape} appears in the JSON message, so terminal escaping leaked \
             into a sink that is not a terminal"
        );
    }
}

#[test]
fn a_hostile_document_name_is_escaped_too() {
    // The message is not the only document-controlled string on the line. A
    // path is chosen by whoever wrote the tree, and it is interpolated into
    // every location this binary prints.
    let directory = scratch("hostile-name");
    let path = write(
        &directory,
        "or\u{202e}ders.yaml",
        "version: 1.0.0\napiVersion: v3.1.0\nkind: DataContract\nid: x\nstatus: active\n",
    );

    let output = conform(&["validate", &path]);
    assert!(!output.stdout.contains('\u{202e}'), "{}", output.stdout);
    assert!(output.stdout.contains("or<U+202E>ders.yaml"));

    let (json, _) = conform_json(&["validate", &path]);
    let document = json["diagnostics"][0]["location"]["document"]
        .as_str()
        .expect("a document name");
    assert!(
        document.contains('\u{202e}'),
        "the JSON location must name the file as it really is, so a consumer can open it"
    );
}

#[test]
fn the_payload_really_does_reach_an_odcl_diagnostic() {
    // The negative control for the two ODCL tests below, and also the proof
    // that the routing reached `conform-lexicon` at all: an `ODCL205` can only
    // have come from that adapter.
    let directory = scratch("hostile-lexicon-control");
    let path = write(&directory, "legacy.yaml", &hostile_lexicon());
    let (json, _) = conform_json(&["validate", &path]);

    let carrying: Vec<&serde_json::Value> = json["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .filter(|d| d["message"].as_str().is_some_and(|m| m.contains(HOSTILE)))
        .collect();

    assert_eq!(
        carrying.len(),
        1,
        "expected exactly one diagnostic quoting the document's `info.status`; \
         the adapter's hygiene rule may have stopped interpolating it"
    );
    assert_eq!(carrying[0]["code"], serde_json::json!("ODCL205"));
}

#[test]
fn no_control_or_bidi_character_from_an_odcl_document_survives_into_the_terminal() {
    let directory = scratch("hostile-lexicon-human");
    let path = write(&directory, "legacy.yaml", &hostile_lexicon());
    let output = conform(&["validate", &path]);

    for character in FORBIDDEN {
        assert!(
            !output.stdout.contains(character),
            "U+{:04X} reached stdout from an ODCL document:\n{}",
            character as u32,
            output.stdout.escape_default()
        );
    }

    // Neutralised, not deleted — same obligation, same evidence.
    for expected in ["<U+202E>", "<U+001B>", "<U+0007>", "<U+200B>"] {
        assert!(
            output.stdout.contains(expected),
            "{expected} is missing, so the payload was dropped rather than shown"
        );
    }
    assert!(output.stdout.contains("act<U+202E>evi<U+001B>[31mtcani"));
}

#[test]
fn json_receives_the_bytes_the_odcl_document_really_held() {
    let directory = scratch("hostile-lexicon-json");
    let path = write(&directory, "legacy.yaml", &hostile_lexicon());
    let (json, _) = conform_json(&["validate", &path]);

    let message = json["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .find_map(|d| d["message"].as_str().filter(|_| d["code"] == "ODCL205"))
        .expect("the ODCL status rule reported something");

    assert!(
        message.contains(HOSTILE),
        "the payload was altered on its way into JSON: {}",
        message.escape_default()
    );

    // The opposite direction, which is the half a renderer gets wrong by
    // escaping once too often. A consumer reading `<U+202E>` where the
    // document held one character has been handed corrupted data.
    for terminal_escape in ["<U+202E>", "<U+001B>", "<U+0007>", "<U+200B>"] {
        assert!(
            !message.contains(terminal_escape),
            "{terminal_escape} appears in the ODCL JSON message, so terminal escaping leaked \
             into a sink that is not a terminal"
        );
    }
}
