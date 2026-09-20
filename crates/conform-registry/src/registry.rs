//! Loading `specs.toml`, and reporting a malformed one the way every other
//! validator in this family reports a malformed document.

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use conform_core::{ConformanceReport, Diagnostic, DocumentId, Location, SpecRef};
use serde::Deserialize;
use toml::Spanned;

use crate::codes;
use crate::entry::SpecEntry;

/// The registry file format version this crate understands.
///
/// A file declaring anything else is refused rather than read optimistically:
/// a provenance record half-understood is worse than one not read at all.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// The reference every diagnostic from this crate is raised under.
#[must_use]
pub fn spec_ref() -> SpecRef {
    SpecRef::new("conform-registry/specs.toml").with_version(SUPPORTED_SCHEMA_VERSION.to_string())
}

/// The deserialization target. Separate from [`Registry`] because the public
/// type carries things the file does not: where it was loaded from, and which
/// line each entry starts on.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    schema_version: u32,
    #[serde(default)]
    spec: Vec<Spanned<SpecEntry>>,
}

/// Every specification this repository conforms to, and the provenance of the
/// bytes it conforms against.
///
/// Three operations, deliberately separate:
///
/// | Call | Touches the filesystem | Answers |
/// |---|---|---|
/// | [`load_path`](Self::load_path) / [`load_str`](Self::load_str) | reads the registry only | is this a registry at all? |
/// | [`validate`](Self::validate) | no | does it record what a registry must record? |
/// | [`verify`](Self::verify) | reads each artefact | do the bytes still match? |
///
/// The split matters because they fail for unrelated reasons and a caller
/// usually wants them apart: a CI job may validate on every build but verify
/// only where the artefacts are checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    schema_version: u32,
    entries: Vec<SpecEntry>,
    /// One-based line each entry's `[[spec]]` header sits on, parallel to
    /// `entries`. Kept so a diagnostic can point at a line in a file a human
    /// is about to edit, rather than at an index they would have to count out.
    lines: Vec<u32>,
    source: DocumentId,
    root: PathBuf,
}

impl Registry {
    /// Read a registry from a file.
    ///
    /// [`vendored_path`](SpecEntry::vendored_path) is resolved relative to the
    /// file's own directory, so a registry at the repository root describes
    /// repository-relative paths and moves with the repository.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadError`] carrying a [`ConformanceReport`] if the file
    /// cannot be read, is not TOML, does not have the shape of a registry, or
    /// declares a `schema_version` this crate does not understand.
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
    /// `source` names the document in any diagnostic raised against it — a
    /// path, a URL, or something synthetic like `<test>`.
    ///
    /// The root for resolving [`vendored_path`](SpecEntry::vendored_path) is
    /// the process's current directory; use [`with_root`](Self::with_root) to
    /// say otherwise.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadError`] carrying a [`ConformanceReport`] if the text is
    /// not TOML, does not have the shape of a registry, or declares a
    /// `schema_version` this crate does not understand.
    pub fn load_str(text: &str, source: impl Into<DocumentId>) -> Result<Self, LoadError> {
        let source = source.into();

        let parsed: RegistryFile = toml::from_str(text).map_err(|error| {
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

        if parsed.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(LoadError::single(
                Diagnostic::error(
                    codes::SCHEMA_VERSION,
                    Location::document(source).with_pointer("/schema_version"),
                    format!(
                        "registry declares schema_version {}, and this crate understands {SUPPORTED_SCHEMA_VERSION}",
                        parsed.schema_version
                    ),
                )
                .with_help(
                    "upgrade conform-registry, or read the file with a version of it that \
                     understands this format",
                )
                .with_spec_ref(spec_ref()),
            ));
        }

        let mut entries = Vec::with_capacity(parsed.spec.len());
        let mut lines = Vec::with_capacity(parsed.spec.len());
        for spanned in parsed.spec {
            let (line, _) = line_and_column(text, spanned.span().start);
            lines.push(line);
            entries.push(spanned.into_inner());
        }

        Ok(Self {
            schema_version: parsed.schema_version,
            entries,
            lines,
            source,
            root: PathBuf::new(),
        })
    }

    /// Set the directory [`vendored_path`](SpecEntry::vendored_path) is
    /// resolved against.
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

    /// Every entry, in file order.
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

    /// The directory [`vendored_path`](SpecEntry::vendored_path) resolves
    /// against.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The absolute-or-relative path of one entry's artefact, as this crate
    /// will look for it.
    #[must_use]
    pub fn artefact_path(&self, entry: &SpecEntry) -> PathBuf {
        self.root.join(&entry.vendored_path)
    }

    /// Where an entry sits in the registry file: the document, the line its
    /// `[[spec]]` header is on, and a pointer to one of its fields.
    #[must_use]
    pub(crate) fn entry_location(&self, index: usize, field: &str) -> Location {
        let mut location = Location::document(self.source.clone());
        if let Some(&line) = self.lines.get(index) {
            location = location.with_line(line);
        }
        location.with_pointer(format!("/spec/{index}/{field}"))
    }
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

/// Saturating `usize` → `u32`, because a line number past four billion is not
/// worth a wider type on [`Location`].
fn truncate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// A registry that could not be read at all.
///
/// Carries a [`ConformanceReport`] rather than a string, so a caller renders a
/// load failure through exactly the same path as a conformance failure — the
/// reason this crate depends on `conform-core` in the first place.
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
