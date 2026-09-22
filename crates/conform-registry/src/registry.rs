//! Loading `specs.toml`, and reporting a malformed one the way every other
//! validator in this family reports a malformed document.

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use conform_core::{ConformanceReport, Diagnostic, DocumentId, Location, SpecRef};
use serde::Deserialize;
use toml::Spanned;

use crate::codes;
use crate::entry::{PinnedVersion, Poll, SpecEntry};

/// The registry file format versions this crate understands.
///
/// Version 1 is the original flat format (one `[[spec]]` per version).
/// Version 2 introduced `[[spec.pin]]` for multiple pinned versions per
/// standard.
pub const SUPPORTED_SCHEMA_VERSIONS: &[u32] = &[1, 2];

/// The reference every diagnostic from this crate is raised under.
#[must_use]
pub fn spec_ref() -> SpecRef {
    SpecRef::new("conform-registry/specs.toml").with_version("2".to_string())
}

// ── Private deserialization types ──────────────────────────────────────

/// Schema version 2: standard-level identity with nested `[[spec.pin]]`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpecV2 {
    id: String,
    name: String,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    steward: Option<String>,
    #[serde(default)]
    licence: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pin: Vec<PinnedVersion>,
}

/// Schema version 1: the original flat format, one entry per version.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpecV1 {
    id: String,
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    steward: Option<String>,
    #[serde(default)]
    licence: Option<String>,
    #[serde(default)]
    pinned_ref: Option<String>,
    vendored_path: String,
    sha256: String,
    fetched_at: String,
    #[serde(default)]
    poll: Option<Poll>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFileV2 {
    schema_version: u32,
    #[serde(default)]
    spec: Vec<Spanned<RawSpecV2>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFileV1 {
    schema_version: u32,
    #[serde(default)]
    spec: Vec<Spanned<RawSpecV1>>,
}

impl RawSpecV1 {
    fn into_entry(self) -> SpecEntry {
        SpecEntry {
            id: self.id,
            name: self.name,
            homepage: self.homepage,
            repository: self.repository,
            steward: self.steward,
            licence: self.licence,
            notes: self.notes,
            versions: vec![PinnedVersion {
                version: self.version,
                pinned_ref: self.pinned_ref,
                vendored_path: self.vendored_path,
                sha256: self.sha256,
                fetched_at: self.fetched_at,
                poll: self.poll,
                notes: None,
            }],
        }
    }
}

impl RawSpecV2 {
    fn into_entry(self) -> SpecEntry {
        SpecEntry {
            id: self.id,
            name: self.name,
            homepage: self.homepage,
            repository: self.repository,
            steward: self.steward,
            licence: self.licence,
            notes: self.notes,
            versions: self.pin,
        }
    }
}

// ── The public registry type ──────────────────────────────────────────

/// Every specification this repository conforms to, and the provenance of the
/// bytes it conforms against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    schema_version: u32,
    entries: Vec<SpecEntry>,
    /// One-based line each entry's `[[spec]]` header sits on, parallel to
    /// `entries`.
    lines: Vec<u32>,
    source: DocumentId,
    root: PathBuf,
}

impl Registry {
    /// Read a registry from a file.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadError`] if the file cannot be read, is not TOML, does
    /// not have the shape of a registry, or declares a `schema_version` this
    /// crate does not understand.
    pub fn load_path(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let source = DocumentId::new(path.display().to_string());
        let text = fs::read_to_string(path).map_err(|error| {
            LoadError::single(Diagnostic::error(
                codes::UNREADABLE,
                Location::document(source.clone()),
                format!("cannot read the registry: {error}"),
            ))
        })?;

        let root = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        Self::load_str(&text, source).map(|registry| registry.with_root(root))
    }

    /// Read a registry from text already in hand.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadError`] if the text is not TOML, does not have the
    /// shape of a registry, or declares a `schema_version` this crate does
    /// not understand.
    pub fn load_str(text: &str, source: impl Into<DocumentId>) -> Result<Self, LoadError> {
        let source = source.into();

        // Peek at the schema version to pick the right deserialization path.
        let version = peek_schema_version(text, &source)?;

        if !SUPPORTED_SCHEMA_VERSIONS.contains(&version) {
            return Err(LoadError::single(
                Diagnostic::error(
                    codes::SCHEMA_VERSION,
                    Location::document(source).with_pointer("/schema_version"),
                    format!(
                        "registry declares schema_version {version}, and this crate understands {}",
                        SUPPORTED_SCHEMA_VERSIONS
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
                .with_help(
                    "upgrade conform-registry, or read the file with a version of it that \
                     understands this format",
                )
                .with_spec_ref(spec_ref()),
            ));
        }

        match version {
            1 => load_v1(text, source),
            2 => load_v2(text, source),
            _ => unreachable!(),
        }
    }

    /// Set the directory vendored paths are resolved against.
    #[must_use]
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }

    /// The format version the file declared.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Every entry, in file order. One entry per standard.
    #[must_use]
    pub fn entries(&self) -> &[SpecEntry] {
        &self.entries
    }

    /// The entry with this identifier, if the registry holds one.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&SpecEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// What the registry was loaded from, as named in diagnostics.
    #[must_use]
    pub const fn source(&self) -> &DocumentId {
        &self.source
    }

    /// The directory vendored paths resolve against.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The absolute-or-relative path of one version's artefact.
    #[must_use]
    pub fn artefact_path(&self, version: &PinnedVersion) -> PathBuf {
        self.root.join(&version.vendored_path)
    }

    /// Where an entry sits in the registry file.
    #[must_use]
    pub(crate) fn entry_location(&self, index: usize, field: &str) -> Location {
        let mut location = Location::document(self.source.clone());
        if let Some(&line) = self.lines.get(index) {
            location = location.with_line(line);
        }
        location.with_pointer(format!("/spec/{index}/{field}"))
    }

    /// Location for a specific version within an entry.
    #[must_use]
    pub(crate) fn version_location(
        &self,
        entry_index: usize,
        version_index: usize,
        field: &str,
    ) -> Location {
        let mut location = Location::document(self.source.clone());
        if let Some(&line) = self.lines.get(entry_index) {
            location = location.with_line(line);
        }
        location.with_pointer(format!("/spec/{entry_index}/pin/{version_index}/{field}"))
    }
}

// ── Version-specific loaders ──────────────────────────────────────────

fn peek_schema_version(text: &str, source: &DocumentId) -> Result<u32, LoadError> {
    #[derive(Deserialize)]
    struct Peek {
        schema_version: u32,
    }
    let peek: Peek = toml::from_str(text).map_err(|error| {
        let mut location = Location::document(source.clone());
        if let Some(span) = error.span() {
            let (line, column) = line_and_column(text, span.start);
            location = location.with_line(line).with_column(column);
        }
        LoadError::single(
            Diagnostic::error(codes::MALFORMED, location, error.message().to_owned())
                .with_spec_ref(spec_ref()),
        )
    })?;
    Ok(peek.schema_version)
}

fn load_v1(text: &str, source: DocumentId) -> Result<Registry, LoadError> {
    let parsed: RegistryFileV1 = toml::from_str(text).map_err(|error| {
        let mut location = Location::document(source.clone());
        if let Some(span) = error.span() {
            let (line, column) = line_and_column(text, span.start);
            location = location.with_line(line).with_column(column);
        }
        LoadError::single(
            Diagnostic::error(codes::MALFORMED, location, error.message().to_owned())
                .with_spec_ref(spec_ref()),
        )
    })?;

    let mut entries = Vec::with_capacity(parsed.spec.len());
    let mut lines = Vec::with_capacity(parsed.spec.len());
    for spanned in parsed.spec {
        let (line, _) = line_and_column(text, spanned.span().start);
        lines.push(line);
        entries.push(spanned.into_inner().into_entry());
    }

    Ok(Registry {
        schema_version: parsed.schema_version,
        entries,
        lines,
        source,
        root: PathBuf::new(),
    })
}

fn load_v2(text: &str, source: DocumentId) -> Result<Registry, LoadError> {
    let parsed: RegistryFileV2 = toml::from_str(text).map_err(|error| {
        let mut location = Location::document(source.clone());
        if let Some(span) = error.span() {
            let (line, column) = line_and_column(text, span.start);
            location = location.with_line(line).with_column(column);
        }
        LoadError::single(
            Diagnostic::error(codes::MALFORMED, location, error.message().to_owned())
                .with_spec_ref(spec_ref()),
        )
    })?;

    let mut entries = Vec::with_capacity(parsed.spec.len());
    let mut lines = Vec::with_capacity(parsed.spec.len());
    for spanned in parsed.spec {
        let (line, _) = line_and_column(text, spanned.span().start);
        lines.push(line);
        entries.push(spanned.into_inner().into_entry());
    }

    Ok(Registry {
        schema_version: parsed.schema_version,
        entries,
        lines,
        source,
        root: PathBuf::new(),
    })
}

/// Whether a vendored path is one this crate will follow: relative, and
/// staying inside the registry's own directory.
pub(crate) fn is_repo_relative(path: &str) -> bool {
    let path = Path::new(path);
    path.is_relative()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
}

/// One-based line and column of a byte offset.
fn line_and_column(text: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.lines().count().max(1);
    let column = before
        .rfind('\n')
        .map_or(offset, |newline| offset - newline - 1)
        + 1;
    (truncate(line), truncate(column))
}

/// Saturating `usize` → `u32`.
fn truncate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// A registry that could not be read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    report: ConformanceReport,
}

impl LoadError {
    /// Wrap one diagnostic.
    fn single(diagnostic: Diagnostic) -> Self {
        let mut report = ConformanceReport::new();
        report.push(diagnostic);
        Self { report }
    }

    /// The diagnostics explaining the failure.
    #[must_use]
    pub const fn report(&self) -> &ConformanceReport {
        &self.report
    }

    /// Take the diagnostics, to merge into a larger report.
    #[must_use]
    pub fn into_report(self) -> ConformanceReport {
        self.report
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.report.diagnostics() {
            [] => f.write_str("registry could not be loaded"),
            [first, rest @ ..] => {
                write!(f, "{first}")?;
                if !rest.is_empty() {
                    write!(f, " (and {} more)", rest.len())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for LoadError {}
