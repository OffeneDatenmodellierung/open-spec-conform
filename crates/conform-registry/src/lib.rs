//! Machine-readable provenance for vendored specification artefacts.
//!
//! A repository that validates documents against a published standard ends up
//! holding a copy of that standard's schema. The copy is the problem. It is a
//! snapshot of a moving document, and unless something records *which* moment
//! it is a snapshot of, nobody can answer the only question that matters about
//! it: is this still what upstream says?
//!
//! This crate is that something. [`specs.toml`](Registry) records, for each
//! vendored artefact, where it came from, who stewards it, what licence it
//! carries, which immutable upstream revision it was taken at, and the
//! SHA-256 of the exact bytes on disk.
//!
//! One standard, many versions. A [`SpecEntry`] describes a specification
//! (identity, steward, licence) and carries one or more [`PinnedVersion`]s,
//! each a vendored artefact at one immutable upstream revision.
//!
//! # The three questions, deliberately separate
//!
//! ```no_run
//! use conform_core::GatePolicy;
//! use conform_registry::Registry;
//!
//! // 1. Is this a registry at all? Reads the registry file, nothing else.
//! let registry = Registry::load_path("specs.toml")?;
//!
//! // 2. Does it record what a registry must record? Pure: no I/O, no clock.
//! let rules = registry.validate();
//!
//! // 3. Do the bytes still match? Re-hashes every artefact on disk.
//! let integrity = registry.verify();
//!
//! assert!(!rules.should_gate(GatePolicy::default()));
//! assert!(!integrity.should_gate(GatePolicy::default()));
//! # Ok::<(), conform_registry::LoadError>(())
//! ```
//!
//! # Offline by construction
//!
//! Nothing here performs network access.

pub mod codes;

mod entry;
mod registry;
mod rules;
mod verify;

pub use entry::{MOVING_REFS, PinnedVersion, Poll, SpecEntry, is_moving_ref};
pub use registry::{LoadError, Registry, SUPPORTED_SCHEMA_VERSIONS, spec_ref};
pub use rules::ProvenanceRules;
pub use verify::{sha256_hex, verify_bytes};
