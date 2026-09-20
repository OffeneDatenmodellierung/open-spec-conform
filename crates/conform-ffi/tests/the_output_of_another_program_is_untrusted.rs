//! Cargo's output is text somebody else wrote, and is read as such.
//!
//! # Why this file exists
//!
//! The C smoke test asks cargo where the static library is and what else to
//! link against it. For one release those answers were scraped out of rustc's
//! *rendered* note on stderr, and on CI — where GitHub Actions sets
//! `CARGO_TERM_COLOR: always` — that note is wrapped in ANSI:
//!
//! ```text
//! ESC[1m ESC[92m note ESC[0m ESC[1m : native-static-libs: … -lc -lm ESC[0m
//! ```
//!
//! The trailing reset ended up inside the last token and was handed to the
//! linker, which replied `library 'm[0m' not found`. On the Linux runner the
//! list ends `-lc`, so the job log read `cannot find -lc^[[0m`.
//!
//! It is worth being blunt about the shape of that. This crate family's CLI
//! has an entire module, with three screens of justification, about
//! neutralising ANSI escapes so a hostile *document* cannot rewrite an
//! operator's terminal. The escape that actually did damage came from our own
//! build tooling, arrived somewhere nobody was looking, and was executed.
//!
//! # What this file holds
//!
//! Three properties of `support::cargo_output`, each with literal `ESC` bytes
//! in the fixture rather than a description of them:
//!
//! 1. the structured route yields exactly the flags rustc named;
//! 2. a token carrying an escape is **refused**, not silently repaired;
//! 3. rendered text is no longer a route at all, so the old code path cannot
//!    come back by accident.
//!
//! It is deliberately **not** behind `--features c-smoke`. It needs no C
//! toolchain and no linker, and the property it asserts — that another
//! program's output is untrusted — should be checked by a plain
//! `cargo test --workspace`, on every platform, every time. A test of a guard
//! that only runs where the guard is least likely to be needed is a test that
//! will be absent on the day it matters.

mod support;

use support::cargo_output::{Unreadable, is_safe_token, linkage, native_static_libs, staticlib};

/// A `compiler-artifact` record for the static library, as cargo emits it.
const ARTEFACT: &str = r#"{"reason":"compiler-artifact","target":{"name":"conform_ffi","kind":["staticlib"]},"filenames":["/w/target/debug/libconform_ffi.a","/w/target/debug/libconform_ffi.d"]}"#;

/// A `compiler-message` record carrying the note: a clean `message`, and a
/// `rendered` sibling that is only fit for showing a human.
///
/// The `rendered` half is colourised here, as
/// `--message-format=json-diagnostic-rendered-ansi` really produces and as a
/// future cargo could decide to under `CARGO_TERM_COLOR=always`. That is
/// deliberate: it means every test below that reads a *value* out of this
/// fixture also fails if somebody switches the parser back to the rendered
/// field, rather than only the one test that is explicitly about escapes.
/// The escapes are written `\u001b` because that is how JSON has to carry a
/// control character — the decoded string holds the real byte.
const NOTE: &str = r#"{"reason":"compiler-message","message":{"level":"note","message":"native-static-libs: -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc","rendered":"\u001b[1m\u001b[92mnote\u001b[0m\u001b[1m: native-static-libs: -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc\u001b[0m\n\n"}}"#;

/// The same record if the colour ever reached the structured field.
///
/// The escape is written the way JSON has to carry one — `\u001b`, six
/// characters — because a raw `ESC` byte inside a JSON string is malformed and
/// `serde_json` refuses the whole line. That is itself a small mercy and is
/// asserted separately below; it is not the guard, because after decoding
/// *this* fixture the Rust `&str` holds a real `ESC` byte, which is exactly
/// what would have reached the linker.
const NOTE_WITH_ESCAPE: &str = r#"{"reason":"compiler-message","message":{"level":"note","message":"native-static-libs: -lpthread -ldl -lc\u001b[0m","rendered":"whatever"}}"#;

/// A record whose `message` field holds a raw, unescaped `ESC`, which is not
/// legal JSON. Written through Rust's own escape so that reading this file is
/// not an exercise in trusting your terminal.
fn note_with_raw_escape() -> String {
    format!(
        r#"{{"reason":"compiler-message","message":{{"level":"note","message":"native-static-libs: -lc{}[0m"}}}}"#,
        '\u{1b}'
    )
}

/// The actual bytes rustc wrote to stderr on CI, reconstructed. This is what
/// the previous version of the parser was reading, and it is included so that
/// "we no longer read rendered text" is asserted rather than assumed.
fn rendered_stderr_line() -> String {
    format!(
        "{esc}[1m{esc}[92mnote{esc}[0m{esc}[1m: native-static-libs: -lgcc_s -lutil -lrt \
         -lpthread -lm -ldl -lc{esc}[0m",
        esc = '\u{1b}'
    )
}

#[test]
fn the_structured_note_yields_exactly_the_flags_rustc_named() {
    let stream = format!("{ARTEFACT}\n{NOTE}\n");
    let flags = native_static_libs(&stream).expect("the note is in the stream");

    // In order, complete, and unaltered. The order matters: rustc says on the
    // line above this note that the order and any duplication can be
    // significant, so a parser that sorted or de-duplicated would be wrong in
    // a way nothing else here would catch.
    assert_eq!(
        flags,
        [
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc"
        ]
    );
}

#[test]
fn the_artefact_path_comes_from_the_structured_record() {
    let stream = format!("{ARTEFACT}\n{NOTE}\n");
    let both = linkage(&stream).expect("both answers are in the stream");

    assert_eq!(
        both.archive.to_str(),
        Some("/w/target/debug/libconform_ffi.a")
    );
    assert_eq!(both.native.len(), 7);
}

#[test]
fn an_escape_inside_a_flag_is_refused_rather_than_repaired() {
    // The assertion this whole file is for. Stripping the escape and carrying
    // on would also have avoided the outage, and would have been worse: it
    // would have hidden the fact that something rendered was being read as
    // though it were structured.
    let stream = format!("{ARTEFACT}\n{NOTE_WITH_ESCAPE}\n");
    let refused = native_static_libs(&stream).expect_err("an escape must not pass");

    let Unreadable::UnsafeToken { shown } = &refused else {
        panic!("expected a refusal naming the token, got {refused:?}");
    };

    // Named, so a maintainer can see which token was wrong...
    assert!(shown.contains("-lc"), "{shown}");
    // ...and shown printably, because a guard that refuses an escape and then
    // writes it to a terminal has protected nothing.
    assert!(
        !shown.contains('\u{1b}'),
        "the refusal put a raw ESC in its own message"
    );
    assert!(
        shown.contains("\\u{1b}") || shown.contains("\\x1b"),
        "{shown}"
    );
}

#[test]
fn a_record_that_is_not_json_cannot_smuggle_anything_through() {
    // A raw `ESC` inside a JSON string is malformed, so the record is not a
    // record and is skipped. What matters is where that leaves the caller: with
    // `NoNativeLibs`, which is a refusal, rather than with a half-read list. A
    // parser that fell back to reading the line as text on a JSON error would
    // have reopened the exact hole this file is about.
    let stream = format!("{ARTEFACT}\n{}\n", note_with_raw_escape());
    assert_eq!(
        native_static_libs(&stream),
        Err(Unreadable::NoNativeLibs),
        "a malformed record must leave nothing behind"
    );
}

#[test]
fn the_refusal_is_not_the_only_thing_standing_between_this_and_the_linker() {
    // The guard is a second line of defence; the first is not reading rendered
    // text at all. This asserts the first: hand the parser precisely what the
    // old one was reading, and it finds nothing rather than finding something
    // wrong.
    let refused = native_static_libs(&rendered_stderr_line())
        .expect_err("rendered text is not a message stream");
    assert_eq!(refused, Unreadable::NoNativeLibs);

    // And the same for a stream that simply has no note in it, so the two
    // cases are the same answer rather than one of them being an accident.
    assert_eq!(native_static_libs(ARTEFACT), Err(Unreadable::NoNativeLibs));
}

#[test]
fn an_absent_artefact_is_an_error_and_not_an_empty_path() {
    assert_eq!(staticlib(NOTE), Err(Unreadable::NoArchive));
    assert_eq!(staticlib(""), Err(Unreadable::NoArchive));
}

#[test]
fn the_allow_list_admits_what_every_platform_actually_emits() {
    // A negative test that only ever sees `-lc` would pass with an allow-list
    // of exactly `['-', 'l', 'c']`, so the positive half is worth stating:
    // these are the real shapes, from the three platforms this can build for.
    for token in [
        "-lpthread",                    // Linux
        "-lgcc_s",                      // Linux, with an underscore
        "-framework",                   // macOS
        "CoreFoundation",               // macOS, a bare name
        "kernel32.lib",                 // Windows
        "-L/usr/lib/aarch64-linux-gnu", // a search path
        "-Wl,--as-needed",              // a comma
        "/usr/lib/libSystem.B.tbd",     // an absolute path
    ] {
        assert!(is_safe_token(token), "{token} should be allowed");
    }

    // And the things that must never reach a linker from parsed text.
    for token in [
        "-lc\u{1b}[0m", // the one that actually happened
        "-lc\u{7}",     // BEL
        "-lc\u{202e}",  // a bidirectional override
        "-lc;rm -rf /", // a shell metacharacter
        "-lc$(whoami)", // a substitution
        "-lc`id`",      // another
        "-lc\"",        // a quote
        "-lc\n-lother", // a newline smuggling a second flag
        "",             // nothing at all
    ] {
        assert!(
            !is_safe_token(token),
            "{} should be refused",
            token.escape_default()
        );
    }
}
