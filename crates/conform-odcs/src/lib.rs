//! Diagnostic-rich conformance validation for **Open Data Contract Standard**
//! documents.
//!
//! # Why this exists
//!
//! ODCS documents in this estate were already being validated. The function
//! doing it looks like this:
//!
//! ```text
//! pub fn validate_odcs_internal(content: &str) -> Result<(), String>
//! ```
//!
//! That signature is the problem, and it is a problem no amount of care inside
//! the function can fix. A `Result<(), String>` can hold **one** finding. It
//! carries no severity, so "you misspelled a key" and "this is not a data
//! contract" arrive identically. It carries no stable code, so a CI job that
//! wants to suppress one known issue has to match on prose. It carries no
//! location, so an editor cannot underline anything. And because the
//! underlying JSON Schema call short-circuits on the first violation, a
//! document with twelve faults reports one, gets fixed, reports the next, and
//! costs twelve round trips.
//!
//! This crate keeps that function's **verdict** and recovers everything it
//! threw away. A document with twelve faults produces twelve diagnostics, each
//! with a stable code, a severity and a JSON Pointer.
//!
//! # The verdict is deliberately unchanged
//!
//! Errors here are **exactly** the published schema's findings, plus two
//! intake failures (the document does not parse; the document is empty).
//! Everything this crate adds beyond the schema — see [`rules`] — is a
//! *warning*, and the default [`GatePolicy`](conform_core::GatePolicy) gates
//! on errors alone. So this validator passes and fails the same documents the
//! schema does, by construction.
//!
//! That is not a limitation; it is the property that makes the change safe to
//! adopt. `tests/oracle_agreement.rs` holds this crate's verdict against the
//! pre-existing `validate_odcs_internal`'s over a fixture corpus and fails on
//! any disagreement that is not recorded, explained and proven to still
//! diverge.
//!
//! # Where the schema comes from
//!
//! Not from a hard-coded path. [`OdcsValidator::from_registry`] resolves the
//! schema through [`conform_registry`], which means the bytes are re-hashed
//! against the digest `specs.toml` records for them *before* anything is
//! validated against them. A schema that has drifted is refused, loudly,
//! rather than used to issue confident verdicts nobody can trace to a
//! published standard.
//!
//! # Example
//!
//! ```no_run
//! use conform_core::{GatePolicy, Severity, Validator};
//! use conform_odcs::{Document, OdcsValidator};
//!
//! let validator = OdcsValidator::from_registry_path("specs.toml")?;
//!
//! let report = validator.validate(&Document::new(
//!     "contracts/orders.yaml",
//!     "apiVersion: v3.1.0\nkind: DataContract\n",
//! ));
//!
//! // Three required properties are absent, and each is its own finding —
//! // the thing `Result<(), String>` has no way to express.
//! for diagnostic in report.at_or_above(Severity::Error) {
//!     println!("{diagnostic}");
//! }
//! assert!(report.should_gate(GatePolicy::default()));
//! # Ok::<(), conform_odcs::SchemaError>(())
//! ```
//!
//! # Known limitation: locations are pointers, not lines
//!
//! A schema violation is located by JSON Pointer
//! ([`Location::pointer`](conform_core::Location::pointer)), not by line and
//! column. The document is parsed into a span-free value tree, so by the time
//! the schema objects to something there is no record of which line it came
//! from. Only a parse failure — where the parser reports its own position —
//! carries a line and column today.
//!
//! This is stated rather than papered over because a pointer is genuinely
//! weaker than a line for an editor integration. Carrying spans through the
//! parse is the work that would fix it, and it is not in 0.1.0.

// Modules are private and their contents re-exported: the public API is flat,
// so a consumer's `use` paths do not change when a type moves file.
pub mod codes;
pub mod rules;

mod document;
mod schema;
mod validator;

pub use document::Document;
pub use validator::{OdcsValidator, SPEC_ID, SchemaError};
