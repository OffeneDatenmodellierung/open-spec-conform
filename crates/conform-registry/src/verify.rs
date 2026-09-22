//! Do the bytes still say what the registry says they say?

use std::fmt::Write as _;
use std::fs;

use conform_core::{ConformanceReport, Diagnostic, DocumentId, Location, Severity};
use sha2::{Digest, Sha256};

use crate::codes;
use crate::entry::PinnedVersion;
use crate::registry::{Registry, spec_ref};

impl Registry {
    /// Re-hash every vendored artefact of every version and compare it to the
    /// recorded digest.
    #[must_use]
    pub fn verify(&self) -> ConformanceReport {
        self.entries()
            .iter()
            .enumerate()
            .flat_map(|(entry_index, entry)| {
                entry
                    .versions
                    .iter()
                    .enumerate()
                    .map(move |(version_index, version)| {
                        self.verify_version(entry_index, version_index, version)
                    })
            })
            .collect()
    }

    /// Re-hash one version's artefact.
    #[must_use]
    pub fn verify_version(
        &self,
        entry_index: usize,
        version_index: usize,
        version: &PinnedVersion,
    ) -> Diagnostic {
        let path = self.artefact_path(version);
        let location = Location::document(path.display().to_string())
            .with_pointer(format!("/spec/{entry_index}/pin/{version_index}/sha256"));

        let entry_id = self
            .entries()
            .get(entry_index)
            .map_or("?", |e| e.id.as_str());
        let label = version
            .version
            .as_deref()
            .map_or_else(|| entry_id.to_owned(), |v| format!("{entry_id}@{v}"));

        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Diagnostic::error(
                    codes::ARTEFACT_MISSING,
                    location,
                    format!(
                        "`{label}` records `vendored_path = \"{}\"`, and there is no such file",
                        version.vendored_path
                    ),
                )
                .with_help(
                    "restore the artefact, or remove the version — a registry entry for bytes \
                     that are not here describes nothing",
                )
                .with_spec_ref(spec_ref());
            }
            Err(error) => {
                return Diagnostic::error(
                    codes::ARTEFACT_UNREADABLE,
                    location,
                    format!("cannot read the artefact for `{label}`: {error}"),
                )
                .with_spec_ref(spec_ref());
            }
        };

        compare(&label, version, &bytes, location)
    }
}

/// Re-hash bytes already in hand against what the registry records for a
/// pinned version.
#[must_use]
pub fn verify_bytes(
    entry_index: usize,
    version_index: usize,
    label: &str,
    version: &PinnedVersion,
    bytes: &[u8],
    document: impl Into<DocumentId>,
) -> Diagnostic {
    let location = Location::document(document)
        .with_pointer(format!("/spec/{entry_index}/pin/{version_index}/sha256"));
    compare(label, version, bytes, location)
}

fn compare(label: &str, version: &PinnedVersion, bytes: &[u8], location: Location) -> Diagnostic {
    let actual = sha256_hex(bytes);
    if actual == version.sha256.trim() {
        Diagnostic::new(
            Severity::Info,
            codes::ARTEFACT_VERIFIED,
            location,
            format!(
                "`{label}` matches its recorded digest ({} bytes)",
                bytes.len()
            ),
        )
        .with_spec_ref(spec_ref())
    } else {
        Diagnostic::error(
            codes::SHA256_MISMATCH,
            location,
            format!(
                "`{label}` hashes to {actual}, and the registry records {}",
                version.sha256.trim()
            ),
        )
        .with_help(
            "either the artefact was edited — in which case it is no longer the upstream \
             document the entry claims — or the registry was updated without re-hashing it",
        )
        .with_spec_ref(spec_ref())
    }
}

/// SHA-256 of some bytes, as 64 lower-case hex digits.
///
/// ```
/// use conform_registry::sha256_hex;
///
/// // The empty-string vector from FIPS 180-4.
/// assert_eq!(
///     sha256_hex(b""),
///     "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
/// );
/// ```
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}
