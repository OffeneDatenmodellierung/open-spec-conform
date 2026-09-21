//! The envelope that crosses the boundary.
//!
//! # Why JSON and not structs
//!
//! This is the single most consequential decision in the crate, so it is
//! written down here rather than left to a commit message.
//!
//! A struct-passing C ABI would put `Diagnostic`'s field list into every
//! downstream binding's compiled artefacts. Adding one field to a
//! `Diagnostic` — a thing that will happen, because that type is the
//! vocabulary five other crates are still growing into — would then be an ABI
//! break for C, for Python, for Node and for WASM simultaneously, and would
//! break them *silently*, because a C program that was compiled against the
//! old layout links happily against the new library and reads the wrong
//! bytes.
//!
//! JSON absorbs that. The ABI is six functions whose signatures never mention
//! a diagnostic; the shape of a diagnostic is data, versioned by
//! [`SCHEMA_VERSION`], and a consumer that does not know a new field ignores
//! it. The cost is a serialise and a parse per document, which is nothing
//! next to compiling a JSON Schema.
//!
//! # Escaping: this envelope is JSON-encoded and nothing else
//!
//! Every adapter in this family quotes the document it found a fault in, and
//! none of them sanitises what it quotes, because escaping is not idempotent
//! and has to happen exactly once, at the point of display. `conform-cli` is a
//! terminal and therefore does it; **this crate is not a terminal and
//! therefore must not**. JSON string encoding is already the correct and
//! complete escaping for this sink, and applying a terminal's neutralisation
//! on top would corrupt the data — a consumer would read `<U+202E>` where the
//! document held one character.
//!
//! That is the same rule `conform-cli`'s `--json` follows, stated in
//! `crates/conform-cli/tests/hostile_text_is_neutralised.rs`, and
//! `tests/hostile_text_crosses_as_json.rs` holds this crate to it.
//! Neutralising for a terminal is the job of whatever renders this envelope,
//! and that is never this crate.

use conform_core::{ConformanceReport, Diagnostic, Location, Severity, SpecRef};
use serde::Serialize;

/// The version of the envelope below.
///
/// Bumped when a field is removed or changes meaning; **not** bumped when one
/// is added, because adding is what the JSON boundary exists to make cheap.
/// A consumer should refuse a `schema_version` it does not know.
pub const SCHEMA_VERSION: u32 = 1;

/// One validation, in full.
///
/// Field-for-field compatible with the `diagnostics` array of `conform`'s
/// `--json` envelope, deliberately: a consumer that can already read a CLI
/// report can read this one, and the two cannot drift into two spellings of
/// one fact without somebody noticing.
#[derive(Debug, Serialize)]
pub struct Envelope {
    /// The version of this envelope's shape. See [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Which library produced it.
    pub tool: Tool,
    /// The standard the document was checked against.
    pub spec: Reference,
    /// What was checked, and what was found in it.
    pub document: Document,
    /// Every finding, in the order the adapter raised it.
    pub diagnostics: Vec<Finding>,
}

impl Envelope {
    /// Build the envelope for one document's report.
    #[must_use]
    pub fn of(document_id: &str, spec: &SpecRef, report: &ConformanceReport) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            tool: Tool {
                name: env!("CARGO_PKG_NAME"),
                version: env!("CARGO_PKG_VERSION"),
            },
            spec: Reference::of(spec),
            document: Document {
                id: document_id.to_owned(),
                worst_severity: report.worst_severity().map(Severity::as_str),
                diagnostics: report.len(),
                error: report.count(Severity::Error),
                warning: report.count(Severity::Warning),
                info: report.count(Severity::Info),
            },
            diagnostics: report.iter().map(Finding::of).collect(),
        }
    }
}

/// Which library produced a report.
#[derive(Debug, Serialize)]
pub struct Tool {
    /// Always `conform-ffi`.
    pub name: &'static str,
    /// This crate's version, not the ABI's. See
    /// [`CONFORM_ABI_VERSION`](crate::CONFORM_ABI_VERSION).
    pub version: &'static str,
}

/// What was checked, and the shape of what was found in it.
#[derive(Debug, Serialize)]
pub struct Document {
    /// The name the caller gave it, which is also how its diagnostics locate
    /// themselves.
    pub id: String,
    /// The worst severity found, or `null` if nothing was found.
    pub worst_severity: Option<&'static str>,
    /// How many findings in total.
    pub diagnostics: usize,
    /// How many errors.
    pub error: usize,
    /// How many warnings.
    pub warning: usize,
    /// How many information notes.
    pub info: usize,
}

/// One diagnostic, in full.
#[derive(Debug, Serialize)]
pub struct Finding {
    /// `error`, `warning` or `info`.
    pub severity: &'static str,
    /// The adapter's stable code. The string downstream tooling matches on.
    pub code: String,
    /// What was found, quoting the document. JSON-encoded and **not**
    /// terminal-escaped; see this module's documentation.
    pub message: String,
    /// What to do about it, when the adapter says.
    pub help: Option<String>,
    /// Where it is.
    pub location: Where,
    /// Which standard it was raised under.
    pub spec_ref: Option<Reference>,
}

impl Finding {
    fn of(diagnostic: &Diagnostic) -> Self {
        Self {
            severity: diagnostic.severity.as_str(),
            code: diagnostic.code.as_str().to_owned(),
            message: diagnostic.message.clone(),
            help: diagnostic.help.clone(),
            location: Where::of(&diagnostic.location),
            spec_ref: diagnostic.spec_ref.as_ref().map(Reference::of),
        }
    }
}

/// Where a diagnostic is.
///
/// `line` and `column` are `null` for most findings and that is honest rather
/// than lossy: a JSON Schema violation is located by pointer, because the
/// document is parsed into a span-free value tree and by the time the schema
/// objects there is no line left to report.
#[derive(Debug, Serialize)]
pub struct Where {
    /// Which document — the id the caller passed to the validate call.
    pub document: String,
    /// One-based line, when the adapter's parser could say.
    pub line: Option<u32>,
    /// One-based column, when the adapter's parser could say.
    pub column: Option<u32>,
    /// Structural pointer within the document, when there is one.
    pub pointer: Option<String>,
}

impl Where {
    fn of(location: &Location) -> Self {
        Self {
            document: location.document.as_str().to_owned(),
            line: location.line,
            column: location.column,
            pointer: location
                .pointer
                .as_ref()
                .map(|pointer| pointer.as_str().to_owned()),
        }
    }
}

/// Which standard something was raised under.
///
/// `id` is a registry identifier, so a consumer can join a finding to
/// `specs.toml` and reach the upstream link, the pin and the licence.
#[derive(Debug, Serialize)]
pub struct Reference {
    /// The registry identifier, e.g. `odcs`.
    pub id: String,
    /// The version of the standard, when the adapter recorded one.
    pub version: Option<String>,
    /// The clause or section, when the rule points at one.
    pub section: Option<String>,
}

impl Reference {
    fn of(spec_ref: &SpecRef) -> Self {
        Self {
            id: spec_ref.id.clone(),
            version: spec_ref.version.clone(),
            section: spec_ref.section.clone(),
        }
    }
}
