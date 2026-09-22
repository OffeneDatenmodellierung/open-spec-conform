//! One registry entry: a standard and its pinned versions.

use serde::Deserialize;

/// What a registry entry records about one vendored specification.
///
/// The fields split into two groups:
///
/// - **Identity** — [`id`](Self::id), [`name`](Self::name),
///   [`homepage`](Self::homepage), [`repository`](Self::repository),
///   [`steward`](Self::steward), [`licence`](Self::licence). These describe the
///   specification itself and are shared across all pinned versions of it.
///
/// - **Pinned versions** — [`versions`](Self::versions). Each is one vendored
///   artefact at one immutable upstream revision, carrying its own
///   [`vendored_path`](PinnedVersion::vendored_path),
///   [`sha256`](PinnedVersion::sha256), [`pinned_ref`](PinnedVersion::pinned_ref)
///   and [`poll`](PinnedVersion::poll).
///
/// Optional does not mean unremarked. [`Registry::validate`](crate::Registry::validate)
/// raises a diagnostic for each absent provenance field, and an entry that has
/// any absence at all must carry [`notes`](Self::notes) explaining it — an
/// unexplained gap is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecEntry {
    /// Stable, short identifier for the specification, unique within one
    /// registry. This is what a `SpecRef` on a diagnostic points at.
    pub id: String,

    /// The specification's full name, as its steward writes it.
    pub name: String,

    /// Canonical human-facing documentation for the specification.
    pub homepage: Option<String>,

    /// Source repository the artefact is published from.
    pub repository: Option<String>,

    /// The organisation or project that maintains the specification.
    pub steward: Option<String>,

    /// The licence the artefact is published under, as an SPDX identifier
    /// where one applies.
    pub licence: Option<String>,

    /// Anything a reader needs in order to trust the entry at the standard
    /// level: shared provenance facts, why a field is absent.
    pub notes: Option<String>,

    /// One or more pinned versions of this specification.
    pub versions: Vec<PinnedVersion>,
}

impl SpecEntry {
    /// The provenance fields this entry's identity does not record, by name,
    /// in a stable order.
    #[must_use]
    pub fn provenance_gaps(&self) -> Vec<&'static str> {
        [
            ("homepage", self.homepage.as_ref()),
            ("repository", self.repository.as_ref()),
            ("steward", self.steward.as_ref()),
            ("licence", self.licence.as_ref()),
        ]
        .into_iter()
        .filter(|(_, value)| value.is_none_or(|v| v.trim().is_empty()))
        .map(|(field, _)| field)
        .collect()
    }

    /// The default version: the first non-draft version in file order, or the
    /// first version if all are drafts.
    ///
    /// "Draft" is detected by common pre-release markers in the version string
    /// (`dev`, `alpha`, `beta`, `rc`, `pre`, `SNAPSHOT`). A version with no
    /// version string at all is not considered a draft.
    #[must_use]
    pub fn default_version(&self) -> &PinnedVersion {
        self.versions
            .iter()
            .find(|v| !v.is_draft())
            .unwrap_or(&self.versions[0])
    }

    /// The index of the default version within [`versions`](Self::versions).
    #[must_use]
    pub fn default_version_index(&self) -> usize {
        self.versions
            .iter()
            .position(|v| !v.is_draft())
            .unwrap_or(0)
    }

    /// The version with this version string, if the entry carries one.
    #[must_use]
    pub fn find_version(&self, version: &str) -> Option<&PinnedVersion> {
        self.versions.iter().find(|v| {
            v.version
                .as_deref()
                .is_some_and(|vs| vs.trim() == version.trim())
        })
    }
}

/// One pinned version of a specification: a vendored artefact and the facts
/// about where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedVersion {
    /// The version of the specification these bytes describe.
    #[serde(default)]
    pub version: Option<String>,

    /// The immutable upstream coordinate these bytes were taken from: a
    /// release tag, a version identifier, or a commit SHA.
    ///
    /// Never a moving pointer.
    #[serde(default)]
    pub pinned_ref: Option<String>,

    /// Where the vendored bytes live, relative to the directory holding the
    /// registry file.
    pub vendored_path: String,

    /// SHA-256 of the vendored bytes, lower-case hex.
    pub sha256: String,

    /// The date the bytes at [`vendored_path`](Self::vendored_path) were taken
    /// into this repository, `YYYY-MM-DD`.
    pub fetched_at: String,

    /// How to ask upstream whether something newer exists.
    #[serde(default)]
    pub poll: Option<Poll>,

    /// Version-specific notes: which evidence dated this artefact, what is
    /// known to be uncertain about it.
    #[serde(default)]
    pub notes: Option<String>,
}

impl PinnedVersion {
    /// Whether this version records an immutable upstream pin.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned_ref
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty() && !is_moving_ref(r))
    }

    /// Whether this version string looks like a pre-release or development
    /// snapshot.
    #[must_use]
    pub fn is_draft(&self) -> bool {
        self.version.as_deref().is_some_and(is_draft_version)
    }
}

/// Whether a version string looks like a pre-release or development snapshot.
fn is_draft_version(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    let markers = ["dev", "alpha", "beta", ".rc", "-rc", "pre", "snapshot"];
    markers.iter().any(|marker| lower.contains(marker))
}

/// How to ask upstream whether the pinned artefact has been superseded.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    /// The URL to ask.
    pub endpoint: String,

    /// How to interpret the endpoint.
    #[serde(default)]
    pub method: Option<String>,

    /// Which field of the response carries the upstream ref to compare.
    #[serde(default)]
    pub field: Option<String>,

    /// How often the check is worth running.
    #[serde(default)]
    pub cadence: Option<String>,
}

/// Refs that name "whatever upstream holds right now" rather than one
/// immutable revision.
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
