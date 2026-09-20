//! A format-agnostic conformance harness: the shared vocabulary a family of
//! validators speaks, and nothing else.
//!
//! Several validators under common ownership each parse a different
//! schema-defined document standard and report whether a document conforms.
//! Each of them was independently reinventing the same surrounding machinery:
//! how a finding is shaped, how reporting differs from gating, and how "the
//! thing you referenced is missing" is kept apart from "I never checked".
//! This crate owns exactly that machinery.
//!
//! What it deliberately does **not** own is any knowledge of a specific
//! standard. There is no parser here, no schema, no version constant, not even
//! the name of a standard in a doc comment or an example. Each adapter keeps
//! its own model and rules and implements [`Validator`] over them. A new
//! standard joins by writing an adapter — never by changing this crate.
//!
//! That boundary is enforced by a test, not by review: `tests/` reads this
//! crate's own sources and fails on any mention of a specific standard.
//!
//! # The pieces
//!
//! | Type | What it is |
//! |---|---|
//! | [`Severity`] | `Info` / `Warning` / `Error`, ordered |
//! | [`Location`] | a document, optionally narrowed to line, column, pointer |
//! | [`Diagnostic`] | one finding: severity, stable [`DiagnosticCode`], message, location |
//! | [`ConformanceReport`] | every finding from one run, plus the gate verdict |
//! | [`GatePolicy`] / [`GateVerdict`] | what fails a run, and whether it did |
//! | [`Validator`] | the one trait an adapter implements |
//! | [`Resolution`] | the three-way answer to a cross-reference check |
//! | [`SpecRef`] | which standard, version and clause a finding was raised under |
//!
//! # Two conventions worth reading before using this crate
//!
//! **Reporting and gating are separate.** [`Validator::validate`] returns a
//! report and cannot fail; [`Validator::check`] applies a [`GatePolicy`] to
//! that report and says whether the run fails. A bare validation reports
//! everything and exits zero. This is structural here — `validate` has no way
//! to signal failure — so it cannot erode back into "the command that reports
//! is the command that fails".
//!
//! **Serialization never changes behaviour.** The optional `serde` feature adds
//! `Serialize`/`Deserialize` to these types and does nothing else. Whether a
//! consumer prints text or JSON must not change which diagnostics are produced
//! or whether a report gates; enabling this feature cannot change either,
//! because no gating or reporting code path is feature-conditional. Anything
//! that would make one feature-conditional is a defect.
//!
//! # Example
//!
//! ```
//! use conform_core::{
//!     ConformanceReport, Diagnostic, GatePolicy, Location, Severity, SpecRef, Validator,
//! };
//!
//! struct IdMustBeLowercase;
//!
//! impl Validator for IdMustBeLowercase {
//!     type Document = str;
//!
//!     fn spec(&self) -> SpecRef {
//!         SpecRef::new("example-standard").with_version("1.0")
//!     }
//!
//!     fn validate(&self, id: &str) -> ConformanceReport {
//!         let mut report = ConformanceReport::new();
//!         if id.chars().any(char::is_uppercase) {
//!             report.push(
//!                 Diagnostic::error("XYZ001", Location::document(id), "identifier must be lowercase")
//!                     .with_help("lowercase it")
//!                     .with_spec_ref(self.spec()),
//!             );
//!         }
//!         report
//!     }
//! }
//!
//! let report = IdMustBeLowercase.validate("Orders");
//! assert_eq!(report.worst_severity(), Some(Severity::Error));
//! assert!(report.should_gate(GatePolicy::default()));
//! assert!(!report.should_gate(GatePolicy::report_only()));
//! ```
//!
//! # Dependencies
//!
//! Zero non-`std` runtime dependencies by default; `serde` behind an
//! off-by-default feature is the only one this crate may ever take. Everything
//! in this family sits on top of it, so its dependency tree is everyone's.

// Modules are private and their contents re-exported: the public API is flat,
// so a consumer's `use` paths do not change when a type moves file.
mod diagnostic;
mod location;
mod report;
mod resolution;
mod severity;
mod spec_ref;
mod validator;

pub use diagnostic::{Diagnostic, DiagnosticCode};
pub use location::{DocumentId, Location, Pointer};
pub use report::{ConformanceReport, GatePolicy, GateVerdict};
pub use resolution::{NotInspectedReason, Resolution};
pub use severity::Severity;
pub use spec_ref::SpecRef;
pub use validator::Validator;
