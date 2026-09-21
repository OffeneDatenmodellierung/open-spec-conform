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
//! SHA-256 of the exact bytes on disk. Everything the crate does is in service
//! of two invariants:
//!
//! - **Nothing may be pinned to a moving target.** `latest`, `main`, `HEAD`
//!   and their relatives name whatever upstream holds today, so a copy pinned
//!   to one of them cannot be checked against anything, ever.
//!   [`Registry::validate`] raises an error on them, and a test in this crate
//!   fails the build if one reaches the real registry.
//! - **A recorded unknown beats a plausible guess.** Every provenance field is
//!   optional, because upstream sometimes genuinely does not publish a licence
//!   or a homepage. Leaving one out is reported, and an entry with any gap in
//!   it must carry `notes` explaining what was looked at and what it did not
//!   say. What the format has no way to express is a confident-looking value
//!   nobody checked.
//!
//! Malformed input is reported as [`conform_core::Diagnostic`]s — the same
//! type every validator in this family emits — rather than as a panic or a
//! bare string, so a broken registry renders through exactly the same path as
//! a broken document.
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
//! Nothing here performs network access. An entry may carry a [`Poll`]
//! endpoint saying *how* to ask upstream whether something newer exists, but
//! asking is a scheduled job that opens an issue — never a check that fails
//! somebody's unrelated pull request, because a gate that cries wolf is a gate
//! that gets switched off (plan §3.3).

pub mod codes;

mod entry;
mod registry;
mod rules;
mod verify;

pub use entry::{MOVING_REFS, Poll, SpecEntry, is_moving_ref};
pub use registry::{LoadError, Registry, SUPPORTED_SCHEMA_VERSION, spec_ref};
pub use rules::ProvenanceRules;
pub use verify::{sha256_hex, verify_bytes};
