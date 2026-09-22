//! The `--json` rendering: one [`Run`], as a versioned envelope.
//!
//! # Versioned from the first release
//!
//! `schema_version` is `1` and it is there on day one, before anybody
//! consumes it, because the alternative is discovering the need for it at the
//! moment the first consumer breaks. A reader that does not understand the
//! version it finds should refuse rather than guess — which is the same rule
//! `conform-registry` applies to `specs.toml`, for the same reason.
//!
//! # This envelope is not `conform-core`'s serde shape
//!
//! `conform-core` has a `serde` feature, and this crate deliberately does not
//! enable it. The types below are *this* crate's, mapped from the harness
//! types by hand. Two reasons, both about not lying to consumers later:
//!
//! - The envelope is the compatibility contract. Deriving it from the
//!   harness's internal layout would make a field rename in `Diagnostic` a
//!   breaking change for every dashboard downstream, which is exactly what
//!   `schema_version` exists to prevent.
//! - `conform-core` carries zero dependencies by default (NFR-001), and it
//!   stays that way because nothing above it reaches down and switches its
//!   optional one on.
//!
//! # Absence is represented as absence
//!
//! Every optional provenance field is emitted as JSON `null` when the registry
//! does not record it — never as `""`, never omitted, and never filled in with
//! something plausible. Three of the seven entries in this repository's
//! registry record no homepage and three record no licence; a consumer must be
//! able to tell that from the JSON, because "nobody wrote it down" is the
//! finding. Substituting an empty string would turn a recorded unknown into an
//! apparent answer, which is the one failure the whole catalogue exists to
//! prevent.
//!
//! # No terminal escaping here
//!
//! [`crate::escape`] is for terminals. JSON string encoding is the correct
//! escaping for this sink, `serde_json` applies it, and applying both would
//! corrupt the data: a consumer would read `<U+202E>` where the document held
//! one character. The values below are the bytes the document really had.

use std::io::{self, Write};

use conform_core::{Diagnostic, GatePolicy, GateVerdict, Location, Severity, SpecRef};
use serde::Serialize;

use crate::human::{TOOL_NAME, TOOL_VERSION};
use crate::model::{self, DocumentOutcome, Run, SpecSummary};

/// The version of this envelope's format.
///
/// Bumped only when a consumer that understood the old shape would misread the
/// new one. Adding a field does not bump it; changing what a field means does.
pub const SCHEMA_VERSION: u32 = 1;

/// Render a run as a single JSON object, pretty-printed and newline-terminated.
///
/// # Errors
///
/// Whatever the underlying writer returns, or a serialisation failure — which
/// cannot happen for these types, and is propagated rather than unwrapped
/// because a binary that panics on its own output is not a good citizen in a
/// pipeline.
pub fn render(run: &Run, out: &mut dyn Write) -> io::Result<()> {
    let envelope = Envelope::of(run);
    serde_json::to_writer_pretty(&mut *out, &envelope)?;
    writeln!(out)
}

/// Where this run's registry came from.
///
/// `kind` is the stable word a consumer branches on — `explicit`,
/// `discovered` or `embedded` — and `path` is the file it came from, or
/// `null` when there was no file because the bytes are inside the binary.
/// Absence is represented as absence here for the same reason it is
/// everywhere else in this envelope: a plausible-looking path that resolves to
/// nothing is worse than an honest `null`.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryOrigin {
    /// `explicit`, `discovered` or `embedded`.
    pub kind: &'static str,
    /// The `specs.toml` this came from, or `null` when it was embedded.
    pub path: Option<String>,
}

impl RegistryOrigin {
    /// Map the model's origin into the envelope's.
    #[must_use]
    pub fn of(origin: &model::RegistryOrigin) -> Self {
        Self {
            kind: origin.kind(),
            path: origin.path().map(|path| path.display().to_string()),
        }
    }
}

/// One run, as machine-readable as it gets.
#[derive(Debug, Serialize)]
pub struct Envelope {
    /// The format version of this object. See [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Which tool produced it, and which version of that tool.
    pub tool: Tool,
    /// Which subcommand was run.
    pub command: &'static str,
    /// The registry the specifications below were resolved through, as the
    /// human rendering names it.
    pub registry: String,
    /// Where that registry came from, and whether it was a file at all.
    ///
    /// Additive in `schema_version` 1: a consumer that never read this field
    /// reads `registry` exactly as before. It is here because `registry` alone
    /// cannot answer the question that matters when a binary carries its own
    /// catalogue — *did an embedded copy answer for a repository I thought I
    /// was checking* — and a consumer gating a pipeline needs to branch on
    /// that rather than parse a sentence.
    pub registry_origin: RegistryOrigin,
    /// What was found, in total.
    pub summary: Summary,
    /// What, if anything, that fails — a separate question from the summary,
    /// and separate here for the same reason it is separate in the harness.
    pub gate: Gate,
    /// The process exit code this run produced.
    pub exit_code: i32,
    /// The specifications in scope, with their upstream provenance.
    pub specs: Vec<Spec>,
    /// Everything examined, with per-document counts.
    pub documents: Vec<Document>,
    /// Every diagnostic, flat, in document order. Each carries the document it
    /// belongs to in its location, so a consumer can regroup without needing
    /// the `documents` array.
    pub diagnostics: Vec<Finding>,
}

impl Envelope {
    /// Map a run into the envelope. Reads; never recomputes.
    #[must_use]
    pub fn of(run: &Run) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            tool: Tool {
                name: TOOL_NAME,
                version: TOOL_VERSION,
            },
            command: run.command.as_str(),
            registry: run.registry_origin.describe(),
            registry_origin: RegistryOrigin::of(&run.registry_origin),
            summary: Summary {
                outcome: outcome(run),
                specs: run.specs.len(),
                documents: run.documents.len(),
                diagnostics: run.documents.iter().map(|d| d.report.len()).sum(),
                error: run.count(Severity::Error),
                warning: run.count(Severity::Warning),
                info: run.count(Severity::Info),
            },
            gate: Gate::of(run),
            exit_code: run.exit_code(),
            specs: run.specs.iter().map(Spec::of).collect(),
            documents: run.documents.iter().map(Document::of).collect(),
            diagnostics: run
                .documents
                .iter()
                .flat_map(|document| document.report.iter())
                .map(Finding::of)
                .collect(),
        }
    }
}

/// Which of the three things happened, named rather than left to be inferred
/// from `exit_code`.
fn outcome(run: &Run) -> &'static str {
    if run.unusable {
        "not-performed"
    } else if run.verdict().is_gated() {
        "gated"
    } else {
        "reported"
    }
}

/// What produced this envelope.
#[derive(Debug, Serialize)]
pub struct Tool {
    /// The binary's name.
    pub name: &'static str,
    /// Its version.
    pub version: &'static str,
}

/// What was found.
#[derive(Debug, Serialize)]
pub struct Summary {
    /// `reported`, `gated`, or `not-performed`.
    pub outcome: &'static str,
    /// How many specifications were in scope.
    pub specs: usize,
    /// How many things were examined.
    pub documents: usize,
    /// How many diagnostics in total.
    pub diagnostics: usize,
    /// How many of them were errors.
    pub error: usize,
    /// How many were warnings.
    pub warning: usize,
    /// How many were information.
    pub info: usize,
}

/// What, if anything, fails this run.
///
/// Kept apart from [`Summary`] deliberately. Reporting and gating are two
/// decisions; a consumer that wants to know whether the build should fail
/// reads `gated`, and a consumer that wants to know what is wrong with the
/// documents reads the summary. Merging them is how a warning ends up failing
/// somebody's unrelated pull request.
#[derive(Debug, Serialize)]
pub struct Gate {
    /// `never` or `at-or-above`.
    pub policy: &'static str,
    /// The severity at or above which this policy fails, or `null` when the
    /// policy is `never`.
    pub threshold: Option<&'static str>,
    /// Whether the policy failed on what was found.
    pub gated: bool,
    /// The worst severity in the whole run, or `null` if nothing was found.
    pub worst_severity: Option<&'static str>,
}

impl Gate {
    fn of(run: &Run) -> Self {
        let verdict = run.verdict();
        Self {
            policy: match run.policy {
                GatePolicy::Never => "never",
                GatePolicy::AtOrAbove(_) => "at-or-above",
            },
            threshold: match run.policy {
                GatePolicy::Never => None,
                GatePolicy::AtOrAbove(severity) => Some(severity.as_str()),
            },
            gated: matches!(verdict, GateVerdict::Gated { .. }),
            worst_severity: verdict.worst().map(Severity::as_str),
        }
    }
}

/// One catalogued specification, and where it came from.
#[derive(Debug, Serialize)]
pub struct Spec {
    /// The registry identifier.
    pub id: String,
    /// The specification's full name.
    pub name: String,
    /// The version the vendored bytes describe, or `null`.
    pub version: Option<String>,
    /// Where upstream is. Every field inside may be `null`.
    pub upstream: Upstream,
    /// The bytes in this repository, and whether they still match.
    pub vendored: Vendored,
    /// The provenance fields this entry does not record, by name. Empty when
    /// every question has an answer.
    pub provenance_gaps: Vec<String>,
    /// Whether this binary can validate documents against this specification.
    /// `false` for a catalogued specification with no adapter — which is an
    /// honest gap, not a pass.
    pub has_validator: bool,
}

impl Spec {
    fn of(spec: &SpecSummary) -> Self {
        Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            version: spec.version.clone(),
            upstream: Upstream {
                homepage: spec.homepage.clone(),
                repository: spec.repository.clone(),
                pinned_ref: spec.pinned_ref.clone(),
                steward: spec.steward.clone(),
                licence: spec.licence.clone(),
                link: spec.upstream_link().map(str::to_owned),
            },
            vendored: Vendored {
                path: spec.vendored_path.clone(),
                sha256: spec.sha256.clone(),
                fetched_at: spec.fetched_at.clone(),
                verify: spec.verify.as_str(),
                verify_code: spec.verify_diagnostic.code.as_str().to_owned(),
                verify_message: spec.verify_diagnostic.message.clone(),
            },
            provenance_gaps: spec.provenance_gaps.clone(),
            has_validator: spec.has_adapter,
        }
    }
}

/// The canonical source a consumer can link back to.
///
/// Every field is nullable and every `null` is load-bearing: it means the
/// registry records no answer, and the entry's `notes` say what was looked at
/// and what it did not say. A `null` here is evidence.
#[derive(Debug, Serialize)]
pub struct Upstream {
    /// Canonical human-facing documentation.
    pub homepage: Option<String>,
    /// Source repository.
    pub repository: Option<String>,
    /// The immutable upstream coordinate the bytes were taken from.
    pub pinned_ref: Option<String>,
    /// The organisation or project that maintains the specification.
    pub steward: Option<String>,
    /// The licence the artefact is published under.
    pub licence: Option<String>,
    /// The single best link to show a human: `homepage`, else `repository`,
    /// else `null`. Provided so a consumer that only wants "the link" does not
    /// have to re-derive that preference and get it different from the TUI.
    pub link: Option<String>,
}

/// The bytes in this repository, and what re-hashing them said.
#[derive(Debug, Serialize)]
pub struct Vendored {
    /// Where they live, relative to the registry file.
    pub path: String,
    /// The digest the registry records.
    pub sha256: String,
    /// When they entered this repository.
    pub fetched_at: String,
    /// `matched`, `drifted`, `missing` or `unreadable`.
    pub verify: &'static str,
    /// `conform-registry`'s own code for that outcome, kept so a consumer
    /// matches on the registry's stable string rather than on this crate's.
    pub verify_code: String,
    /// The registry's own explanation, unparaphrased.
    pub verify_message: String,
}

/// One thing that was examined.
#[derive(Debug, Serialize)]
pub struct Document {
    /// What it is called, exactly as its diagnostics name it.
    pub id: String,
    /// The specification it was checked against, or `null` when it was not
    /// checked against one — a finding about the invocation, or about the
    /// registry itself.
    pub spec_id: Option<String>,
    /// The worst severity found in it, or `null` if nothing was found.
    pub worst_severity: Option<&'static str>,
    /// How many errors were found in it.
    pub error: usize,
    /// How many warnings.
    pub warning: usize,
    /// How many information notes.
    pub info: usize,
}

impl Document {
    fn of(document: &DocumentOutcome) -> Self {
        Self {
            id: document.id.clone(),
            spec_id: document.spec_id.clone(),
            worst_severity: document.worst().map(Severity::as_str),
            error: document.report.count(Severity::Error),
            warning: document.report.count(Severity::Warning),
            info: document.report.count(Severity::Info),
        }
    }
}

/// One diagnostic, in full.
#[derive(Debug, Serialize)]
pub struct Finding {
    /// `error`, `warning` or `info`.
    pub severity: &'static str,
    /// The adapter's stable code. The string downstream tooling matches on.
    pub code: String,
    /// What was found, quoting the document. **Not** terminal-escaped: see
    /// this module's documentation.
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
/// objects there is no line left to report. `conform-odcs` says so in its own
/// documentation, and inventing a line here would be worse than admitting it.
#[derive(Debug, Serialize)]
pub struct Where {
    /// Which document.
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

/// Which standard a diagnostic was raised under.
///
/// `id` is a registry identifier, so a consumer can join this to the `specs`
/// array above and reach the upstream link, the pin and the licence. That join
/// is the reason a `spec_ref` is carried at all.
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
