//! The stable codes this binary raises findings under.
//!
//! Stability is the contract `conform-core` asks for: downstream tooling,
//! suppression lists and CI annotations match on these strings, so a code is
//! retired rather than reused, and the message attached to one may be reworded
//! freely while the code may not.
//!
//! There are deliberately few of them. Everything this binary reports about a
//! *document* is the adapter's own diagnostic, forwarded unchanged with the
//! adapter's own code; a `CLI0xx` code is only ever about the **invocation** —
//! a path that is not there, a file nothing here knows how to read, a registry
//! that could not be loaded. Paraphrasing an adapter's finding under a code of
//! our own would break the one link that makes a finding traceable.

/// A path given on the command line does not exist, or could not be read.
pub const PATH_UNREADABLE: &str = "CLI001";

/// A file was found, and nothing here can tell which standard it is written
/// in. Reported rather than skipped: silently ignoring a file is how a
/// document comes to be believed checked when it never was.
pub const UNRECOGNISED_DOCUMENT: &str = "CLI002";

/// The paths given held no document any adapter here validates.
pub const NO_DOCUMENTS: &str = "CLI003";

/// `--spec` names an identifier the registry does not hold.
pub const UNKNOWN_SPEC: &str = "CLI004";

/// `--spec` names a registry entry that has no adapter in this binary — an
/// honest gap rather than a silent pass. `odcl` and `cads` are in the
/// catalogue and have no validator yet.
pub const NO_ADAPTER_FOR_SPEC: &str = "CLI005";

/// **Retired.** No `specs.toml` could be found, so nothing could say where any
/// vendored artefact came from.
///
/// This binary can no longer reach that state: when nothing on disk answers it
/// falls back to the catalogue compiled into it (see [`crate::embedded`]) and
/// says on the `registry:` line that it did. The constant stays, unemitted,
/// because a code is retired rather than reused — anybody still suppressing or
/// matching on `CLI100` must not one day find it attached to something else.
pub const REGISTRY_NOT_FOUND: &str = "CLI100";

/// A document was not checked, because the validator for its standard could
/// not be built — most often because the vendored schema failed its provenance
/// check. Recorded per document rather than once, so "not checked" never looks
/// like "checked and clean".
pub const NOT_CHECKED: &str = "CLI006";
