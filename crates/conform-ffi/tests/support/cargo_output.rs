//! Reading another program's output, on the assumption that it is text rather
//! than data.
//!
//! # The bug this module exists because of
//!
//! The C smoke test needs two facts from cargo: where the static library is,
//! and which system libraries a Rust `staticlib` has to be linked against on
//! this platform. The second used to be taken by scraping rustc's rendered
//! note out of stderr:
//!
//! ```text
//! note: native-static-libs: -liconv -lSystem -lc -lm
//! ```
//!
//! GitHub Actions sets `CARGO_TERM_COLOR: always`, so on CI that line is not
//! what it looks like. It is:
//!
//! ```text
//! ESC[1m ESC[92m note ESC[0m ESC[1m : native-static-libs: … -lc -lm ESC[0m
//! ```
//!
//! — and the trailing reset lands inside the last token. The linker was handed
//! `-lm\x1b[0m` and said, accurately, `library 'm[0m' not found`. On CI's
//! Linux runner the list ends with `-lc` instead, which is why the job log
//! read `cannot find -lc^[[0m`.
//!
//! There is a particular sting in this one. This crate family's CLI exists in
//! part to stop a hostile document's ANSI escapes reaching an operator's
//! terminal; `conform-cli`'s `escape` module has three screens of prose about
//! it. The escape that actually caused damage here came from our own build
//! tooling, went unexamined, and was handed to a linker.
//!
//! # The two things done about it
//!
//! **Stop parsing rendered text.** `--message-format=json` puts that note on
//! *stdout* as a structured `compiler-message` record whose `message` field is
//! the sentence without any decoration, whatever `CARGO_TERM_COLOR` says. The
//! colour, when there is any, lives in a separate `rendered` field that this
//! module only ever uses to make a *failure* readable — never to extract a
//! value from. The old `--message-format=json-render-diagnostics` deliberately
//! does the opposite: it keeps no structured copy and renders to stderr, which
//! is what forced the scraping in the first place.
//!
//! **Then distrust it anyway.** Every token that will become a linker argument
//! is checked against a character allow-list, and anything outside it is
//! refused rather than repaired. Refused, not stripped: an escape arriving
//! here would mean this module's assumption about where its input comes from
//! has quietly stopped being true, and silently cleaning that up would hide
//! the thing worth knowing. That is a different obligation from the renderer's
//! in `conform-cli`, which neutralises because it is about to *display*; this
//! is about to *execute*, and the right answer there is to stop.
//!
//! `tests/the_output_of_another_program_is_untrusted.rs` holds both halves to
//! it with literal `ESC` bytes, so neither can quietly regress.

#![allow(dead_code)]

use std::fmt;
use std::path::{Path, PathBuf};

/// The prefix rustc puts on the note this module is looking for.
const NATIVE_LIBS: &str = "native-static-libs:";

/// What it takes to link a C program against the built library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linkage {
    /// The static library cargo built.
    pub archive: PathBuf,
    /// The system libraries rustc says a `staticlib` needs alongside it, in
    /// the order it gave them — which it warns can matter.
    pub native: Vec<String>,
}

/// Why a cargo message stream could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    /// No `compiler-artifact` record named a static library.
    NoArchive,
    /// No `compiler-message` record carried the `native-static-libs` note.
    ///
    /// Which, since the switch to `--message-format=json`, is also what
    /// happens if somebody points this at rendered stderr again: rendered text
    /// is not a JSON stream, so there is nothing here to find.
    NoNativeLibs,
    /// A token that would have become a linker argument held something this
    /// module will not pass on.
    UnsafeToken {
        /// The offending token, rendered printably. Never the raw bytes: a
        /// module that refuses an escape sequence and then prints it to a
        /// terminal has achieved nothing.
        shown: String,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoArchive => f.write_str(
                "cargo's message stream named no static library. Was it run with \
                 `--message-format=json` and `--crate-type staticlib`?",
            ),
            Self::NoNativeLibs => f.write_str(
                "cargo's message stream carried no `native-static-libs` note. That note is a \
                 structured `compiler-message` under `--message-format=json`; under \
                 `--message-format=json-render-diagnostics` it is rendered to stderr instead and \
                 there is no structured copy to read.",
            ),
            Self::UnsafeToken { shown } => write!(
                f,
                "a library flag held a character this module will not pass to a linker: `{shown}`. \
                 The most likely cause is that something rendered rather than structured is being \
                 parsed — see this module's documentation for the time that happened."
            ),
        }
    }
}

impl std::error::Error for Unreadable {}

/// Whether a token is safe to hand to a linker.
///
/// A character allow-list rather than a pattern for each shape a flag can
/// take, because the shapes vary by platform and the characters do not:
/// `-lpthread` on Linux, `-framework` and `CoreFoundation` on macOS, bare
/// `kernel32.lib` on Windows, and absolute paths anywhere. What none of them
/// contain is a control character, a quote, a shell metacharacter or a space.
#[must_use]
pub fn is_safe_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_./+:=,".contains(character))
}

/// A token as it can be put in a message without doing to the reader's
/// terminal what it would have done to the linker.
#[must_use]
pub fn show(token: &str) -> String {
    token.escape_default().to_string()
}

/// Every JSON record in a cargo message stream, skipping anything that is not
/// one — cargo is entitled to put human-readable lines on stdout and
/// occasionally does.
fn records(stdout: &str) -> impl Iterator<Item = serde_json::Value> + '_ {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
}

/// The static library path out of a cargo message stream.
///
/// # Errors
///
/// [`Unreadable::NoArchive`] if no artefact record named one.
pub fn staticlib(stdout: &str) -> Result<PathBuf, Unreadable> {
    for message in records(stdout) {
        if message["reason"] != "compiler-artifact" || message["target"]["name"] != "conform_ffi" {
            continue;
        }
        let is_staticlib = message["target"]["kind"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "staticlib"));
        if !is_staticlib {
            continue;
        }
        // By extension rather than by position: a `staticlib` artefact record
        // also carries the `.d` depfile, and on Windows the archive is `.lib`.
        let found = message["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .find(|name| {
                Path::new(name).extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("a") || extension.eq_ignore_ascii_case("lib")
                })
            });
        if let Some(found) = found {
            return Ok(PathBuf::from(found));
        }
    }
    Err(Unreadable::NoArchive)
}

/// The system libraries out of a cargo message stream.
///
/// Read from the structured `message` field of the `compiler-message` record,
/// never from its `rendered` sibling — see this module's documentation for
/// what happened the last time the rendered one was trusted. Every token is
/// then checked anyway.
///
/// The order is preserved exactly as rustc gave it, because the line above the
/// note in rustc's own output says the order and any duplication can be
/// significant.
///
/// # Errors
///
/// [`Unreadable::NoNativeLibs`] if the note is not in the stream;
/// [`Unreadable::UnsafeToken`] if any token holds a character this module will
/// not pass to a linker.
pub fn native_static_libs(stdout: &str) -> Result<Vec<String>, Unreadable> {
    let note = records(stdout)
        .filter(|message| message["reason"] == "compiler-message")
        .find_map(|message| {
            let text = message["message"]["message"].as_str()?;
            let (_, libraries) = text.split_once(NATIVE_LIBS)?;
            Some(libraries.to_owned())
        })
        .ok_or(Unreadable::NoNativeLibs)?;

    let mut tokens = Vec::new();
    for token in note.split_whitespace() {
        if !is_safe_token(token) {
            return Err(Unreadable::UnsafeToken { shown: show(token) });
        }
        tokens.push(token.to_owned());
    }
    if tokens.is_empty() {
        return Err(Unreadable::NoNativeLibs);
    }
    Ok(tokens)
}

/// Both answers at once.
///
/// # Errors
///
/// As [`staticlib`] and [`native_static_libs`].
pub fn linkage(stdout: &str) -> Result<Linkage, Unreadable> {
    Ok(Linkage {
        archive: staticlib(stdout)?,
        native: native_static_libs(stdout)?,
    })
}

/// The compiler's own diagnostics, rendered, for a failure message.
///
/// `--message-format=json` stops cargo rendering diagnostics to stderr, so a
/// build failure would otherwise arrive as a wall of JSON. This is the one
/// place the `rendered` field is read, and it is read to *show* a human rather
/// than to extract a value — which is the distinction this whole module is
/// about.
#[must_use]
pub fn rendered_diagnostics(stdout: &str) -> String {
    let mut out = String::new();
    for message in records(stdout) {
        if message["reason"] != "compiler-message" {
            continue;
        }
        if let Some(rendered) = message["message"]["rendered"].as_str() {
            out.push_str(rendered);
        }
    }
    out
}
