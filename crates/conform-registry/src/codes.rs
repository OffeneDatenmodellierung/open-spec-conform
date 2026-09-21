//! The stable codes this crate raises findings under.
//!
//! Stability is the contract `conform-core` asks for: downstream tooling,
//! suppression lists and CI annotations match on these strings, so a code is
//! retired rather than reused, and the message attached to one may be reworded
//! freely while the code may not.
//!
//! `REG0xx` is the registry file itself, `REG1xx` is reading it at all.

/// `pinned_ref` names a moving target — `latest`, `main`, `HEAD` — rather than
/// one immutable revision. The defect this crate exists to prevent.
pub const MOVING_REF: &str = "REG001";

/// The entry records no `pinned_ref` at all, so nothing can say which upstream
/// revision the bytes came from.
pub const UNPINNED: &str = "REG002";

/// A provenance field is absent: nobody recorded the homepage, repository,
/// steward or licence.
pub const PROVENANCE_GAP: &str = "REG003";

/// The entry has a gap and no `notes` explaining it. An unknown is allowed;
/// an unexplained unknown is not.
pub const UNEXPLAINED_GAP: &str = "REG004";

/// Two entries claim the same `id`.
pub const DUPLICATE_ID: &str = "REG005";

/// A field that must always be known is present but blank.
pub const BLANK_FIELD: &str = "REG006";

/// `sha256` is not 64 lower-case hex digits, so it cannot be a SHA-256.
pub const MALFORMED_SHA256: &str = "REG007";

/// `vendored_path` is absolute, or climbs out of the registry's directory —
/// a path only one machine can resolve.
pub const PATH_NOT_REPO_RELATIVE: &str = "REG008";

/// A URL field does not start with `https://`.
pub const INSECURE_URL: &str = "REG009";

/// `fetched_at` is not an ISO-8601 `YYYY-MM-DD` date.
pub const MALFORMED_DATE: &str = "REG010";

/// The artefact named by `vendored_path` is not there.
pub const ARTEFACT_MISSING: &str = "REG020";

/// The artefact is there but could not be read.
pub const ARTEFACT_UNREADABLE: &str = "REG021";

/// The artefact's bytes no longer hash to the recorded `sha256`: either the
/// file changed or the registry did.
pub const SHA256_MISMATCH: &str = "REG022";

/// The artefact's bytes hash to exactly what the registry recorded. Reported
/// as information, because "checked and correct" is a different statement
/// from "not checked" and both look like silence otherwise.
pub const ARTEFACT_VERIFIED: &str = "REG023";

/// The registry file could not be read from disk.
pub const UNREADABLE: &str = "REG100";

/// The registry file is not TOML, or not shaped like a registry.
pub const MALFORMED: &str = "REG101";

/// The registry declares a `schema_version` this crate does not understand.
pub const SCHEMA_VERSION: &str = "REG102";
