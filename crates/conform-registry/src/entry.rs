//! One registry entry: a vendored artefact and everything known about where it
//! came from.

use serde::Deserialize;

/// What a registry entry records about one vendored specification artefact.
///
/// The fields split into three groups, and the split is the point of the type:
///
/// - **Identity** — [`id`](Self::id), [`name`](Self::name). Always present.
/// - **Custody** — [`vendored_path`](Self::vendored_path),
///   [`sha256`](Self::sha256), [`fetched_at`](Self::fetched_at). Always
///   present: these are facts about bytes in this repository, so there is
///   never an excuse for not knowing them.
/// - **Provenance** — [`homepage`](Self::homepage),
///   [`repository`](Self::repository), [`steward`](Self::steward),
///   [`licence`](Self::licence), [`pinned_ref`](Self::pinned_ref). Every one
///   of these is optional, because they are facts about *upstream* and
///   upstream sometimes genuinely does not record them.
///
/// Optional does not mean unremarked. [`Registry::validate`](crate::Registry::validate)
/// raises a diagnostic for each absent provenance field, and an entry that has
/// any absence at all must carry [`notes`](Self::notes) explaining it — an
/// unexplained gap is an error. Omitting a field is therefore a way to record
/// "we do not know", never a way to stay quiet about it. Writing a plausible
/// guess into the field instead is the failure this whole crate exists to make
/// hard to commit.
///
/// The fields are public: this is a record, and an accessor per field would
/// buy nothing. Adding a field is a breaking change, which is the right price.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
// A misspelled key is the exact silent-drift failure this crate exists to
// prevent: `sha_256 = "…"` deserialized leniently would leave the real
// `sha256` missing rather than say so.
#[serde(deny_unknown_fields)]
pub struct SpecEntry {
    /// Stable, short identifier for the specification, unique within one
    /// registry. This is what a `SpecRef` on a diagnostic points at.
    pub id: String,

    /// The specification's full name, as its steward writes it.
    pub name: String,

    /// The version of the specification these bytes describe, when it is
    /// known. Distinct from [`pinned_ref`](Self::pinned_ref): the version is
    /// what the document *says it is*, the pinned ref is the immutable
    /// upstream coordinate the bytes were taken from.
    #[serde(default)]
    pub version: Option<String>,

    /// Canonical human-facing documentation for the specification.
    #[serde(default)]
    pub homepage: Option<String>,

    /// Source repository the artefact is published from.
    #[serde(default)]
    pub repository: Option<String>,

    /// The organisation or project that maintains the specification.
    #[serde(default)]
    pub steward: Option<String>,

    /// The licence the artefact is published under, as an SPDX identifier
    /// where one applies.
    ///
    /// Absent means *not recorded anywhere we could check*, which is a real
    /// and reportable state — vendoring bytes whose licence nobody wrote down
    /// is a problem that only gets harder to fix with age.
    #[serde(default)]
    pub licence: Option<String>,

    /// The immutable upstream coordinate these bytes were taken from: a
    /// release tag, a version identifier, or a commit SHA.
    ///
    /// Never a moving pointer. `latest`, `main` and `HEAD` name whatever
    /// upstream happens to hold today, so a copy pinned to one of them cannot
    /// be checked against anything — which is precisely the defect that
    /// motivated this crate. [`Registry::validate`](crate::Registry::validate)
    /// rejects them.
    #[serde(default)]
    pub pinned_ref: Option<String>,

    /// Where the vendored bytes live, relative to the directory holding the
    /// registry file.
    ///
    /// Relative on purpose: an absolute path, or one that climbs out with
    /// `..`, describes a file that only exists on one machine, and a
    /// provenance record nobody else can verify is not a provenance record.
    pub vendored_path: String,

    /// SHA-256 of the vendored bytes, lower-case hex.
    pub sha256: String,

    /// The date the bytes at [`vendored_path`](Self::vendored_path) were taken
    /// into this repository, `YYYY-MM-DD`.
    pub fetched_at: String,

    /// How to ask upstream whether something newer exists.
    #[serde(default)]
    pub poll: Option<Poll>,

    /// Anything a reader needs in order to trust the entry: which evidence
    /// dated the artefact, why a provenance field is absent, what is known to
    /// be uncertain.
    ///
    /// Mandatory in practice for any entry with a gap in it — see the type
    /// documentation.
    #[serde(default)]
    pub notes: Option<String>,
}

impl SpecEntry {
    /// Whether this entry records an immutable upstream pin.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned_ref
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty() && !is_moving_ref(r))
    }

    /// The provenance fields this entry does not record, by name, in a stable
    /// order.
    ///
    /// The list a reader should be shown before deciding whether to trust the
    /// bytes. Empty means every provenance question has an answer.
    #[must_use]
    pub fn provenance_gaps(&self) -> Vec<&'static str> {
        [
            ("homepage", self.homepage.as_ref()),
            ("repository", self.repository.as_ref()),
            ("steward", self.steward.as_ref()),
            ("licence", self.licence.as_ref()),
            ("pinned_ref", self.pinned_ref.as_ref()),
        ]
        .into_iter()
        .filter(|(_, value)| value.is_none_or(|v| v.trim().is_empty()))
        .map(|(field, _)| field)
        .collect()
    }
}

/// How to ask upstream whether the pinned artefact has been superseded.
///
/// Drift detection reports; it never gates. An upstream release is news, not a
/// defect in the pull request that happens to be open when it lands, so
/// nothing in this crate performs network access and nothing here can fail a
/// build (plan §3.3).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    /// The URL to ask.
    pub endpoint: String,

    /// How to interpret the endpoint: `github-release`, `github-tag`,
    /// `github-commit`, `http-etag` or `manual`. Free-form here; the poller
    /// that eventually reads it owns the vocabulary.
    #[serde(default)]
    pub method: Option<String>,

    /// Which field of the response carries the upstream ref to compare
    /// against [`SpecEntry::pinned_ref`].
    #[serde(default)]
    pub field: Option<String>,

    /// How often the check is worth running.
    #[serde(default)]
    pub cadence: Option<String>,
}

/// Refs that name "whatever upstream holds right now" rather than one
/// immutable revision.
///
/// `latest` is the one that has already cost us — a vendored schema named for
/// it, with no version, no source URL and no way to detect that upstream had
/// moved. The rest are the same mistake wearing different clothes, and they
/// are listed here so the rule generalises instead of fixing one filename.
///
/// Matching is case-insensitive and ignores surrounding whitespace, because
/// `Latest` and `" latest "` are the same defect.
pub const MOVING_REFS: &[&str] = &[
    "latest", "head", "main", "master", "trunk", "default", "stable", "current", "edge", "tip",
    "release", "dev", "next",
];

/// Whether a ref names a moving target rather than one immutable revision.
///
/// ```
/// use conform_registry::is_moving_ref;
///
/// assert!(is_moving_ref("latest"));
/// assert!(is_moving_ref("LATEST"));
/// assert!(is_moving_ref("  Latest  "));
/// assert!(!is_moving_ref("v1.0.0"));
/// assert!(!is_moving_ref("ad30107c31c06aec8a7d5636e0d1058118604e6f"));
/// ```
#[must_use]
pub fn is_moving_ref(candidate: &str) -> bool {
    let candidate = candidate.trim();
    MOVING_REFS
        .iter()
        .any(|moving| candidate.eq_ignore_ascii_case(moving))
}
