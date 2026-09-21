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
//!
//! | Band | Meaning |
//! |---|---|
//! | `CLI0xx` | the invocation — what was asked for, and what could be read |
//! | `CLI1xx` | the registry could not be found at all |
//! | `CLI2xx` | the **corpus** — facts that only exist between documents |
//!
//! # Why the `CLI2xx` band is not a violation of the paragraph above
//!
//! It looks like one. It is the opposite, and the distinction is worth being
//! precise about: a `CLI2xx` finding is one **no adapter could have made**.
//!
//! Every validator in this family is single-document by construction, and that
//! is deliberate — it is what makes a verdict reproducible from one file and
//! nothing else. The cost is that two facts the specifications state outright
//! become unobservable from inside one:
//!
//! - an ODPS `contractId` names an ODCS contract. `conform-odps` holds one
//!   product document and has never seen a contract, so it cannot follow the
//!   reference; it cannot even tell whether anybody *could*.
//! - an ODCS `id` is "a unique identifier used to reduce the risk of dataset
//!   name collisions" — across contracts, which one contract cannot see.
//!
//! So these are not paraphrases of an adapter's finding. They are findings
//! about the *set*, raised by the only component that holds one, and they are
//! warnings and information exclusively: nothing here ever changes whether a
//! document gates.

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

/// No `specs.toml` could be found, so nothing can say where any vendored
/// artefact came from.
pub const REGISTRY_NOT_FOUND: &str = "CLI100";

/// A document was not checked, because the validator for its standard could
/// not be built — most often because the vendored schema failed its provenance
/// check. Recorded per document rather than once, so "not checked" never looks
/// like "checked and clean".
pub const NOT_CHECKED: &str = "CLI006";

// ---------------------------------------------------------------------------
// CLI2xx — the corpus. Facts that exist between documents and nowhere inside
// one, so no single-document adapter can raise them. Warnings and information
// only.
// ---------------------------------------------------------------------------

/// An ODPS port names a `contractId`, this run held ODCS contracts to resolve
/// it against, and none of them declares that `id`.
///
/// A **warning**, never an error, and the reason is the same one every hygiene
/// rule in this family gives: the published ODPS schema types `contractId` as
/// a bare `"type": "string"`, so a document this code fires on is a document
/// the schema accepts, and a validator that failed it would disagree with
/// every other ODPS tool in existence.
///
/// The claim it makes is narrow and it is worth reading exactly: *the
/// contracts this run loaded do not include one with that `id`*. It is never
/// raised when there were no contracts to look at — that case is
/// [`CONTRACT_NOT_INSPECTED`], and keeping the two apart is the whole point of
/// [`conform_core::Resolution`].
pub const CONTRACT_DOES_NOT_EXIST: &str = "CLI200";

/// An ODPS port names a `contractId` and nobody looked for it.
///
/// Information, not a finding. Raised when this run holds no ODCS contract to
/// resolve against — a directory of product files, or `--spec odps` — or when
/// a contract in the run could not be read, which means the corpus is
/// incomplete and "absent" is a claim this binary is not entitled to make.
///
/// Reported rather than passed over in silence, because "I followed this and
/// it was fine" and "I never followed this" are different facts, and a tool
/// that renders them identically is a tool that gets trusted for the wrong
/// reason.
pub const CONTRACT_NOT_INSPECTED: &str = "CLI201";

/// An ODPS port names a `contractId` and a contract in this run declares it.
///
/// Information. Paired with [`CONTRACT_NOT_INSPECTED`], which is what makes
/// either of them meaningful: a reader who sees this knows the link was
/// actually followed.
pub const CONTRACT_RESOLVED: &str = "CLI202";

/// Two ODCS contracts in one run declare the same `id`.
///
/// The schema calls `/properties/id` "a unique identifier used to reduce the
/// risk of dataset name collisions" and then has no way to check it: a JSON
/// Schema validates one instance, and a collision is a fact about two. Raised
/// on **both** contracts, each naming the other, because either of them may be
/// the one that should change and nothing here knows which.
///
/// A warning for the usual reason, and for one more: the two documents may be
/// two copies of the same contract in a monorepo, which is a question for a
/// human rather than a verdict for a gate.
pub const DUPLICATE_CONTRACT_ID: &str = "CLI203";
