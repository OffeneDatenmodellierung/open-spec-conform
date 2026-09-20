//! The crate family, read from the manifests rather than transcribed.
//!
//! # Why this is not a list in the source
//!
//! There are eight crates here today and the page has to name every one of
//! them with its version. A hand-written list would be correct on the day it
//! was written and wrong on the day after the first release, in exactly the
//! way the specification catalogue would be wrong if its upstream links were
//! hand-typed. The failure is the same failure, so the answer is the same
//! answer: read the source of truth.
//!
//! The source of truth for a crate's version is its own `Cargo.toml`, and
//! deliberately not `env!("CARGO_PKG_VERSION")` through a dependency edge.
//! That macro would report the version of a crate this one *depends on*, which
//! would force this crate to depend on all eight — including `conform-ffi`,
//! whose `cdylib` this page has no use for — and would still not reach the
//! `description` field, which the page shows and which no macro exposes for
//! anything but the current crate.
//!
//! # Every crate, not a curated subset
//!
//! [`read_dir`] enumerates the workspace's crate directory. A crate added to
//! the workspace appears on the page without anybody editing this file, and a
//! crate removed from it disappears. The alternative — a list here, filtered
//! against the directory — would let the two drift and would report the drift
//! to nobody.

use std::io;
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use serde::Deserialize;

/// The three keys this crate reads out of a member manifest.
///
/// `#[serde(deny_unknown_fields)]` is deliberately *not* set, unlike
/// `conform-registry`'s registry types: a `Cargo.toml` is full of keys that
/// are none of this module's business, and refusing to read one because it
/// mentions a lint table would be absurd. The registry's strictness is about a
/// file this repository owns and whose misspellings are silent drift; this is
/// about a file cargo owns.
#[derive(Debug, Deserialize)]
struct ManifestFile {
    package: PackageTable,
}

/// The `[package]` table, in the two-and-a-half fields the page shows.
#[derive(Debug, Deserialize)]
struct PackageTable {
    name: String,
    /// Optional in the type, though not in this workspace.
    ///
    /// A member that inherits its version from the workspace would deserialize
    /// here as absent rather than as a string, because the inherited form is
    /// `version.workspace = true` — a table, not a value. That is precisely
    /// the state this workspace forbids (plan §7.2), so representing it as
    /// [`None`] and rendering it as an absence is the honest reading: the
    /// page says the version is not recorded, which is what a reader needs to
    /// know, rather than failing to build or inventing a number.
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    publish: Option<bool>,
}

/// One crate in the family, as the page shows it.
///
/// [`version`](Self::version) and [`description`](Self::description) stay
/// [`Option`] for the reason every provenance field in
/// [`conform_registry::SpecEntry`] does: absent is a state, and a renderer
/// that substituted an empty string would report a fact nobody established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateEntry {
    /// The package name, as cargo knows it.
    pub name: String,
    /// The literal version in the crate's own manifest.
    pub version: Option<String>,
    /// The crate's one-line description.
    pub description: Option<String>,
    /// Whether the crate is published to a registry. `publish = false` in a
    /// manifest means no; absent means yes, which is cargo's own default and
    /// is why this is a `bool` rather than an `Option<bool>`.
    pub published: bool,
    /// The manifest this row was read from, relative to the workspace root.
    pub manifest_path: String,
}

/// Read every member manifest under `crates_dir`, in name order.
///
/// Sorted by name rather than left in directory order, because directory order
/// is filesystem-dependent and a page that reorders itself between two
/// machines is a page whose diffs nobody can read.
///
/// # Errors
///
/// [`ManifestError`] if the directory cannot be listed, or if a manifest in it
/// cannot be read or is not a manifest. A member that will not parse is an
/// error rather than a skipped row: a crate silently missing from the family
/// list is the drift this module exists to prevent.
pub fn read_dir(crates_dir: impl AsRef<Path>) -> Result<Vec<CrateEntry>, ManifestError> {
    let crates_dir = crates_dir.as_ref();

    let listing = fs::read_dir(crates_dir).map_err(|error| ManifestError {
        path: crates_dir.to_path_buf(),
        detail: Detail::Io(error),
    })?;

    let mut manifests = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|error| ManifestError {
            path: crates_dir.to_path_buf(),
            detail: Detail::Io(error),
        })?;
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        }
    }

    let mut crates = manifests
        .iter()
        .map(|path| read_one(path, crates_dir))
        .collect::<Result<Vec<_>, _>>()?;
    crates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(crates)
}

/// Read one manifest.
fn read_one(path: &Path, crates_dir: &Path) -> Result<CrateEntry, ManifestError> {
    let text = fs::read_to_string(path).map_err(|error| ManifestError {
        path: path.to_path_buf(),
        detail: Detail::Io(error),
    })?;

    let manifest: ManifestFile = toml::from_str(&text).map_err(|error| ManifestError {
        path: path.to_path_buf(),
        detail: Detail::Toml(Box::new(error)),
    })?;

    // Relative to the directory holding the crates, then prefixed with that
    // directory's own name, so the page shows `crates/conform-core/Cargo.toml`
    // — a path a reader can paste into an editor — rather than an absolute
    // path that only exists on the machine that built the page.
    let relative = path.strip_prefix(crates_dir).unwrap_or(path);
    let manifest_path = Path::new(crates_dir.file_name().map_or("".as_ref(), |n| n))
        .join(relative)
        .display()
        .to_string();

    Ok(CrateEntry {
        name: manifest.package.name,
        version: present(manifest.package.version),
        description: present(manifest.package.description),
        published: manifest.package.publish.unwrap_or(true),
        manifest_path,
    })
}

/// Fold a present-but-blank value to absent, as
/// [`conform_cli::model::SpecSummary`] does for every provenance field and for
/// the same reason: `Some("")` is not evidence of anything, and representing
/// it as present would make the page claim a fact nobody established.
fn present(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// A manifest that could not be read.
#[derive(Debug)]
pub struct ManifestError {
    path: PathBuf,
    detail: Detail,
}

/// Why a manifest could not be read. Private: callers get the sentence, not
/// the discriminant, because there is nothing useful to branch on.
#[derive(Debug)]
enum Detail {
    Io(io::Error),
    /// Boxed because `toml::de::Error` is large and this variant is the rare
    /// one; an unboxed one would make every `Result` in this module wide.
    Toml(Box<toml::de::Error>),
}

impl ManifestError {
    /// The manifest, or the directory, that could not be read.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.path.display())?;
        match &self.detail {
            Detail::Io(error) => write!(f, "{error}"),
            Detail::Toml(error) => write!(f, "{}", error.message()),
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.detail {
            Detail::Io(error) => Some(error),
            Detail::Toml(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace's own crate directory, found relative to this file so the
    /// test does not depend on where cargo was invoked from.
    fn crates_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/conform-web has a parent")
            .to_path_buf()
    }

    #[test]
    fn every_member_of_this_workspace_is_read() {
        let crates = read_dir(crates_dir()).expect("the workspace's crates directory reads");

        // This crate is in the list, which is the cheapest proof that the
        // reader found the real directory rather than an empty one.
        assert!(
            crates.iter().any(|c| c.name == env!("CARGO_PKG_NAME")),
            "conform-web should appear in its own family list",
        );

        // Deliberately not an equality assertion against a number. A count
        // written here would have to be edited every time the workspace grows,
        // which is the transcription this module exists to avoid — and it
        // would fail for the wrong reason when somebody added a crate.
        assert!(
            crates.len() >= 2,
            "a workspace with one crate in it is a reader that found the wrong directory",
        );
    }

    #[test]
    fn versions_are_literal_in_every_manifest() {
        // Plan §7.2 forbids `version.workspace = true` anywhere in this
        // workspace, and the workspace has no version key to inherit even if
        // somebody tried. A member that had one would deserialize with
        // `version: None` — so this assertion is also the guard that would
        // catch it.
        for member in read_dir(crates_dir()).expect("the crates directory reads") {
            assert!(
                member.version.is_some(),
                "{} records no literal version; `version.workspace = true` \
                 does not build in this workspace",
                member.name,
            );
        }
    }

    #[test]
    fn a_manifest_without_a_package_table_is_an_error_not_an_empty_row() {
        let directory = std::env::temp_dir().join("conform-web-manifest-test");
        let member = directory.join("not-a-crate");
        fs::create_dir_all(&member).expect("the temporary directory is creatable");
        fs::write(member.join("Cargo.toml"), "[workspace]\n").expect("the manifest is writable");

        let outcome = read_dir(&directory);
        assert!(
            outcome.is_err(),
            "a member that will not parse must fail the build, not vanish from the page",
        );

        fs::remove_dir_all(&directory).expect("the temporary directory is removable");
    }

    #[test]
    fn blank_metadata_is_folded_to_absent() {
        assert_eq!(present(Some("   ".to_owned())), None);
        assert_eq!(present(Some(" 0.1.0 ".to_owned())), Some("0.1.0".to_owned()));
        assert_eq!(present(None), None);
    }
}
