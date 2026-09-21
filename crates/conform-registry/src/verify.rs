//! Do the bytes still say what the registry says they say?
//!
//! Recording a SHA-256 and never checking it is a comfort, not a control.
//! This module is the check: re-hash every vendored artefact and report each
//! outcome — matched, drifted, missing, unreadable — as a diagnostic.

use std::fmt::Write as _;
use std::fs;

use conform_core::{ConformanceReport, Diagnostic, Location, Severity};
use sha2::{Digest, Sha256};

use crate::codes;
use crate::entry::SpecEntry;
use crate::registry::{Registry, spec_ref};

impl Registry {
    /// Re-hash every vendored artefact and compare it to the recorded digest.
    ///
    /// Touches the filesystem, and only the filesystem: there is no network
    /// access anywhere in this crate, so this answers "have our bytes changed"
    /// and never "has upstream moved" (plan §3.3 — drift against upstream is
    /// reported by a scheduled poll, and does not gate a build).
    ///
    /// Every entry produces exactly one diagnostic, including the ones that
    /// pass. A silent success and a skipped check are indistinguishable
    /// otherwise, and this crate exists because that distinction went missing
    /// once already.
    #[must_use]
    pub fn verify(&self) -> ConformanceReport {
        self.entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| self.verify_entry(index, entry))
            .collect()
    }

    /// Re-hash one entry's artefact. See [`verify`](Registry::verify).
    #[must_use]
    pub fn verify_entry(&self, index: usize, entry: &SpecEntry) -> Diagnostic {
        let path = self.artefact_path(entry);
        // The artefact is the document a drift diagnostic is *about*; the
        // pointer says which registry field disagrees with it.
        let location = Location::document(path.display().to_string())
            .with_pointer(format!("/spec/{index}/sha256"));

        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Diagnostic::error(
                    codes::ARTEFACT_MISSING,
                    location,
                    format!(
                        "`{}` records `vendored_path = \"{}\"`, and there is no such file",
                        entry.id, entry.vendored_path
                    ),
                )
                .with_help(
                    "restore the artefact, or remove the entry — a registry entry for bytes that \
                     are not here describes nothing",
                )
                .with_spec_ref(spec_ref());
            }
            Err(error) => {
                return Diagnostic::error(
                    codes::ARTEFACT_UNREADABLE,
                    location,
                    format!("cannot read the artefact for `{}`: {error}", entry.id),
                )
                .with_spec_ref(spec_ref());
            }
        };

        let actual = sha256_hex(&bytes);
        if actual == entry.sha256.trim() {
            Diagnostic::new(
                Severity::Info,
                codes::ARTEFACT_VERIFIED,
                location,
                format!(
                    "`{}` matches its recorded digest ({} bytes)",
                    entry.id,
                    bytes.len()
                ),
            )
            .with_spec_ref(spec_ref())
        } else {
            Diagnostic::error(
                codes::SHA256_MISMATCH,
                location,
                format!(
                    "`{}` hashes to {actual}, and the registry records {}",
                    entry.id,
                    entry.sha256.trim()
                ),
            )
            .with_help(
                "either the artefact was edited — in which case it is no longer the upstream \
                 document the entry claims — or the registry was updated without re-hashing it",
            )
            .with_spec_ref(spec_ref())
        }
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
        // Lower-case, fixed width: the digest is compared as text against the
        // registry, so its spelling is part of the format. Writing into a
        // `String` cannot fail, so the result is deliberately discarded.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}
