//! One run's worth of facts, computed once and rendered three ways.
//!
//! This module is the reason the renderers cannot drift apart. A [`Run`] is
//! built by [`crate::engine`] and is the **only** thing the human renderer,
//! the `--json` renderer and the TUI are allowed to read: none of them
//! validates anything, none of them re-derives a count, and none of them
//! decides whether the run gates. A renderer that computed its own diagnostics
//! could disagree with its siblings, which is exactly the failure the equality
//! test in `tests/json_changes_serialisation_only.rs` exists to catch — and
//! the cheapest way to pass that test forever is to leave the renderers with
//! nothing to disagree about.

use std::path::{Path, PathBuf};

use conform_core::{ConformanceReport, Diagnostic, GatePolicy, GateVerdict, Severity};
use conform_registry::SpecEntry;

/// Which subcommand produced a [`Run`].
///
/// Carried so the renderers can label their output without being told twice,
/// and so the `--json` envelope records what was asked as well as what was
/// found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// `conform validate` — documents checked against their standard.
    Validate,
    /// `conform registry list` — the catalogue and its provenance, no checking.
    RegistryList,
    /// `conform registry verify` — the vendored bytes re-hashed.
    RegistryVerify,
}

impl Command {
    /// The stable, machine-readable name of this command, as it appears in the
    /// `--json` envelope.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::RegistryList => "registry list",
            Self::RegistryVerify => "registry verify",
        }
    }
}

/// The four standards this binary has an adapter for.
///
/// The registry holds five entries. `cads` is catalogued, vendored and
/// verified like the rest — there is simply no validator for it yet, and
/// [`crate::codes::NO_ADAPTER_FOR_SPEC`] says so rather than letting a
/// `--spec cads` run report a clean bill of health it never earned.
///
/// Every identifier below is read from the adapter crate's own `SPEC_ID`
/// constant, never typed out here. `conform-lexicon`'s is `odcl` and not
/// `lexicon`, because the registry issues `odcl` and the registry is the
/// single source of truth for what a standard is called; a second spelling in
/// this file is a second source of truth, and one of them would be wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Standard {
    /// Open Data Contract Standard — `kind: DataContract`.
    Odcs,
    /// Open Data Product Standard — `kind: DataProduct`.
    Odps,
    /// Open Knowledge Format — a directory of markdown, not a single file.
    Okf,
    /// Open Data Contract Lexicon — a root `dataContractSpecification` key.
    Odcl,
}

impl Standard {
    /// Every standard with an adapter, in registry order.
    pub const ALL: [Self; 4] = [Self::Odcs, Self::Odps, Self::Okf, Self::Odcl];

    /// The `specs.toml` identifier this standard's schema is registered under.
    #[must_use]
    pub const fn spec_id(self) -> &'static str {
        match self {
            Self::Odcs => conform_odcs::SPEC_ID,
            Self::Odps => conform_odps::SPEC_ID,
            Self::Okf => conform_okf::SPEC_ID,
            Self::Odcl => conform_lexicon::SPEC_ID,
        }
    }

    /// The standard registered under this identifier, if this binary has an
    /// adapter for it.
    #[must_use]
    pub fn from_spec_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.spec_id() == id)
    }

    /// This standard's single-file form, if it has one.
    ///
    /// [`Standard::Okf`] has none, and that is the whole reason
    /// [`FileStandard`] exists.
    #[must_use]
    pub const fn as_file(self) -> Option<FileStandard> {
        match self {
            Self::Odcs => Some(FileStandard::Odcs),
            Self::Odps => Some(FileStandard::Odps),
            Self::Odcl => Some(FileStandard::Odcl),
            Self::Okf => None,
        }
    }
}

/// A standard whose document is one file.
///
/// The distinction is not pedantry, it is what keeps an unreachable branch out
/// of the engine. Three of the four adapters here take a named string; the
/// fourth takes a directory and walks it. A single `Standard` for both means
/// every place that dispatches on "which text validator" has an OKF arm that
/// cannot happen and must still be written — and an arm that cannot happen is
/// an arm nobody tests. Making the invariant a type instead means the compiler
/// checks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStandard {
    /// Open Data Contract Standard.
    Odcs,
    /// Open Data Product Standard.
    Odps,
    /// Open Data Contract Lexicon.
    Odcl,
}

impl FileStandard {
    /// This as one of the four standards.
    #[must_use]
    pub const fn standard(self) -> Standard {
        match self {
            Self::Odcs => Standard::Odcs,
            Self::Odps => Standard::Odps,
            Self::Odcl => Standard::Odcl,
        }
    }
}

/// What re-hashing one vendored artefact said.
///
/// A four-way answer rather than a boolean, because "the file is not there"
/// and "the file is there and has changed" call for different work, and
/// flattening them to `false` is how a missing artefact comes to be reported
/// as a drifted one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerifyStatus {
    /// The bytes hash to exactly what the registry records.
    Matched,
    /// The bytes are there and hash to something else.
    Drifted,
    /// `vendored_path` names a file that does not exist.
    Missing,
    /// The file exists and could not be read.
    Unreadable,
}

impl VerifyStatus {
    /// The stable, machine-readable name, as it appears in the `--json`
    /// envelope.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::Drifted => "drifted",
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
        }
    }

    /// A one-glyph summary for a pane too narrow for the word.
    ///
    /// Never the only encoding of the status — plan §4.2 rules out
    /// colour-only, and glyph-only is the same mistake in a different font.
    /// Everywhere this appears, [`as_str`](Self::as_str) appears beside it.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Matched => "✓",
            Self::Drifted => "✗",
            Self::Missing | Self::Unreadable => "?",
        }
    }

    /// What the registry's own `REG0xx` code for this entry means.
    ///
    /// Read from the code rather than the message, because the code is the
    /// part `conform-registry` promises not to change.
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            conform_registry::codes::ARTEFACT_VERIFIED => Self::Matched,
            conform_registry::codes::ARTEFACT_MISSING => Self::Missing,
            conform_registry::codes::ARTEFACT_UNREADABLE => Self::Unreadable,
            // `SHA256_MISMATCH`, and anything a future version of the registry
            // adds: the bytes are not what was recorded, which is drift.
            _ => Self::Drifted,
        }
    }
}

/// One registry entry, as this binary shows it: the provenance, plus what
/// re-hashing the bytes said.
///
/// Every provenance field stays an [`Option`]. That is the whole point of the
/// registry — a field nobody could determine is *left out*, and a recorded
/// unknown is usable evidence where a plausible-looking guess is not — so a
/// renderer that substituted an empty string here would be undoing the one
/// guarantee the catalogue offers. Blank-but-present is folded to [`None`] to
/// match [`SpecEntry::provenance_gaps`], which treats a whitespace-only value
/// as the gap it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSummary {
    /// The registry identifier, e.g. `odcs`.
    pub id: String,
    /// The specification's full name, as its steward writes it.
    pub name: String,
    /// The version of the specification the vendored bytes describe.
    pub version: Option<String>,
    /// Canonical human-facing documentation. The link the TUI puts one
    /// keystroke away.
    pub homepage: Option<String>,
    /// Source repository the artefact is published from.
    pub repository: Option<String>,
    /// The organisation or project that maintains the specification.
    pub steward: Option<String>,
    /// The licence the artefact is published under, where one is recorded.
    pub licence: Option<String>,
    /// The immutable upstream coordinate the bytes were taken from.
    pub pinned_ref: Option<String>,
    /// Where the vendored bytes live, relative to `specs.toml`.
    pub vendored_path: String,
    /// The digest the registry records for those bytes.
    pub sha256: String,
    /// The date the bytes entered this repository.
    pub fetched_at: String,
    /// The provenance fields this entry does not record, by name.
    pub provenance_gaps: Vec<String>,
    /// What re-hashing the bytes said.
    pub verify: VerifyStatus,
    /// The registry's own diagnostic for that check, kept verbatim so a reader
    /// gets `conform-registry`'s code and explanation rather than a paraphrase.
    pub verify_diagnostic: Diagnostic,
    /// Whether this binary has an adapter that can validate documents against
    /// this specification.
    pub has_adapter: bool,
    /// Everything a reader needs in order to trust the entry.
    pub notes: Option<String>,
}

impl SpecSummary {
    /// Build a summary from a registry entry and its verification diagnostic.
    #[must_use]
    pub fn new(entry: &SpecEntry, verify_diagnostic: Diagnostic) -> Self {
        Self {
            id: entry.id.clone(),
            name: entry.name.clone(),
            version: present(entry.version.as_deref()),
            homepage: present(entry.homepage.as_deref()),
            repository: present(entry.repository.as_deref()),
            steward: present(entry.steward.as_deref()),
            licence: present(entry.licence.as_deref()),
            pinned_ref: present(entry.pinned_ref.as_deref()),
            vendored_path: entry.vendored_path.clone(),
            sha256: entry.sha256.clone(),
            fetched_at: entry.fetched_at.clone(),
            provenance_gaps: entry
                .provenance_gaps()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            verify: VerifyStatus::from_code(verify_diagnostic.code.as_str()),
            verify_diagnostic,
            has_adapter: Standard::from_spec_id(&entry.id).is_some(),
            notes: present(entry.notes.as_deref()),
        }
    }

    /// The best upstream link for this entry: its documentation site if one is
    /// recorded, otherwise its repository, otherwise nothing.
    ///
    /// Nothing is a real answer. Three of the five entries in this
    /// repository's registry record no homepage, and inventing one would be
    /// the failure the catalogue exists to prevent.
    #[must_use]
    pub fn upstream_link(&self) -> Option<&str> {
        self.homepage.as_deref().or(self.repository.as_deref())
    }
}

/// Fold a present-but-blank value to absent.
///
/// `Some("")` and `Some("   ")` are not evidence of anything, and
/// [`SpecEntry::provenance_gaps`] already counts them as gaps; representing
/// them as present here would make the renderers disagree with the registry
/// about what is known.
fn present(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

/// The name findings about the *invocation* are reported under.
///
/// Not a document: a path that was not there, or a set of paths that held
/// nothing checkable, is a fact about the command line. It is given a name so
/// it renders through the same path as everything else, and it is excluded
/// from the "documents examined" count so a failed invocation never reads as
/// though a document had been checked.
pub const COMMAND_LINE: &str = "<command line>";

/// One thing that was examined, and everything found in it.
///
/// "Document" is meant broadly: an ODCS contract, an ODPS product, an ODCL
/// data contract, an OKF bundle directory, the registry file itself, or a
/// vendored artefact that was re-hashed. What they have in common is the only thing that matters here —
/// each is a named thing with a [`ConformanceReport`] about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentOutcome {
    /// What the thing is called, exactly as its diagnostics name it.
    pub id: String,
    /// The registry identifier of the standard it was checked against, when it
    /// was checked against one.
    pub spec_id: Option<String>,
    /// Everything found. Errors first, then warnings, then info.
    pub report: ConformanceReport,
}

impl DocumentOutcome {
    /// A new outcome, with its report already ordered by severity.
    #[must_use]
    pub fn new(id: impl Into<String>, spec_id: Option<String>, report: ConformanceReport) -> Self {
        let mut report = report;
        report.sort_by_severity();
        Self {
            id: id.into(),
            spec_id,
            report,
        }
    }

    /// The worst severity found in this document, if anything was found.
    #[must_use]
    pub fn worst(&self) -> Option<Severity> {
        self.report.worst_severity()
    }

    /// A one-glyph summary of that severity, for a pane too narrow for the
    /// word. Always shown beside the word, never instead of it.
    #[must_use]
    pub fn glyph(&self) -> &'static str {
        match self.worst() {
            Some(Severity::Error) => "✗",
            Some(Severity::Warning) => "⚠",
            _ => "✓",
        }
    }
}

/// Everything one invocation found, before anybody has decided how to show it.
///
/// Built once by [`crate::engine::run`]; read by all three renderers and by
/// Which registry answered this run, and why that one.
///
/// # Precedence, and why it is this way round
///
/// An explicit `--registry` beats a `specs.toml` found on disk, which beats
/// the copy compiled into the binary. Each step down is a step further from
/// what the caller is looking at, so each is only reached when the one above
/// has nothing to say.
///
/// The last step is the one that needs justifying. Somebody standing in a
/// checkout is asking about *that* checkout; a binary that quietly answered
/// from its own compiled-in catalogue would report the specifications it was
/// built with while the reader believed they were seeing the ones in front of
/// them. That is a false green, and it is the failure this whole project
/// exists to prevent — so the embedded copy is reached last, and every
/// renderer says when it was reached.
///
/// # This is not a path
///
/// [`Run::registry_path`] is what a *diagnostic* names as its document. This
/// is where the registry came from, which is a different question: under
/// [`Embedded`](Self::Embedded) there is no file, and a renderer that printed
/// one would be pointing a reader at something that does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryOrigin {
    /// `--registry <path>` named it. The caller was explicit and wins.
    Explicit(PathBuf),
    /// Found by searching upward from the working directory.
    Discovered(PathBuf),
    /// Compiled into this binary, because nothing on disk answered.
    Embedded,
}

impl RegistryOrigin {
    /// The stable word a machine-readable consumer matches on.
    ///
    /// Stable in the sense every code in this crate is stable: a consumer
    /// branches on these three strings, so one is retired rather than
    /// respelled.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Explicit(_) => "explicit",
            Self::Discovered(_) => "discovered",
            Self::Embedded => "embedded",
        }
    }

    /// The file this came from, when it came from one.
    ///
    /// [`None`] for [`Embedded`](Self::Embedded), which is the honest answer:
    /// there is no file. A consumer wanting a path gets an absence rather than
    /// a plausible-looking string that resolves to nothing.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Explicit(path) | Self::Discovered(path) => Some(path),
            Self::Embedded => None,
        }
    }

    /// Whether the bytes behind this registry are inside the binary.
    #[must_use]
    pub const fn is_embedded(&self) -> bool {
        matches!(self, Self::Embedded)
    }

    /// The sentence the `registry:` line shows.
    ///
    /// Says which registry answered *and why it was the one*, because those
    /// are two different things a reader needs and the second is the one that
    /// is easy to leave out. "I found this here" and "I fell back to this
    /// because there was nothing else" support different next actions.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Explicit(path) => format!("{} (--registry)", path.display()),
            Self::Discovered(path) => format!(
                "{} (found by searching upward from the working directory)",
                path.display()
            ),
            Self::Embedded => format!(
                "embedded in {} {} (no specs.toml found on disk; pinned when this version was \
                 published)",
                crate::human::TOOL_NAME,
                crate::human::TOOL_VERSION,
            ),
        }
    }
}

/// nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Which subcommand this was.
    pub command: Command,
    /// The gating policy the caller asked for. Defaults to
    /// [`GatePolicy::Never`]: reporting and gating are separate decisions, and
    /// a bare `validate` reports.
    pub policy: GatePolicy,
    /// What a diagnostic about the registry names as its document — a path
    /// when there is one, and `specs.toml (embedded)` when the bytes are in
    /// the binary.
    pub registry_path: String,
    /// Which registry answered, and why that one. Rendered on the `registry:`
    /// line by every renderer, because a reader must never have to guess
    /// whether an embedded catalogue answered for a repository they thought
    /// they were checking.
    pub registry_origin: RegistryOrigin,
    /// The catalogue, filtered by `--spec` when one was given.
    pub specs: Vec<SpecSummary>,
    /// Everything examined, in the order it was examined.
    pub documents: Vec<DocumentOutcome>,
    /// Set when the run could not be performed at all — a registry that will
    /// not load, a `--spec` naming nothing. The diagnostics explaining it are
    /// in [`documents`](Self::documents) like any others; this only changes
    /// the exit code, because "I could not do the work" and "I did the work
    /// and it passed" must not exit the same way.
    pub unusable: bool,
}

impl Run {
    /// Every diagnostic from every document, in document order.
    ///
    /// The set the equality test compares across renderers.
    #[must_use]
    pub fn report(&self) -> ConformanceReport {
        let mut merged = ConformanceReport::new();
        for document in &self.documents {
            merged.extend(document.report.iter().cloned());
        }
        merged
    }

    /// How many diagnostics of one severity this run found.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.documents
            .iter()
            .map(|document| document.report.count(severity))
            .sum()
    }

    /// The verdict of applying this run's policy to everything it found.
    #[must_use]
    pub fn verdict(&self) -> GateVerdict {
        self.report().gate(self.policy)
    }

    /// The process exit code.
    ///
    /// Three outcomes, deliberately distinguishable:
    ///
    /// | Code | Means |
    /// |---|---|
    /// | `0` | the run happened, and the policy did not fail it |
    /// | `1` | the run happened, and the policy failed it |
    /// | `2` | the run could not happen |
    ///
    /// `0` is what a bare `conform validate` returns over a document full of
    /// errors, and that is not an oversight: **reporting is not gating**.
    /// `conform-core` models the distinction with [`GatePolicy`], every
    /// adapter in this family is built around it, and collapsing it here — by
    /// exiting non-zero because something was *found* — would undo it at the
    /// one place a user meets it. Gating is opt-in, with `--gate` or `--check`.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.unusable {
            return 2;
        }
        self.verdict().exit_code()
    }

    /// The specification summary with this identifier, if the run holds one.
    #[must_use]
    pub fn spec(&self, id: &str) -> Option<&SpecSummary> {
        self.specs.iter().find(|spec| spec.id == id)
    }

    /// The documents checked against one specification, in run order.
    #[must_use]
    pub fn documents_for(&self, spec_id: &str) -> Vec<&DocumentOutcome> {
        self.documents
            .iter()
            .filter(|document| document.spec_id.as_deref() == Some(spec_id))
            .collect()
    }

    /// The documents not attributable to any one specification — a registry
    /// that would not load, a path that was not there.
    #[must_use]
    pub fn unattributed(&self) -> Vec<&DocumentOutcome> {
        self.documents
            .iter()
            .filter(|document| document.spec_id.is_none())
            .collect()
    }
}
