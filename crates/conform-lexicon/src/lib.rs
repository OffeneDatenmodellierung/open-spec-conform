//! Diagnostic-rich conformance validation for **Open Data Contract Lexicon**
//! documents.
//!
//! # Naming, stated once so it does not have to be guessed
//!
//! This estate calls the standard ODCL, "Open Data Contract Language", and
//! this crate is `conform-lexicon`. The document itself calls it neither: its
//! `title` is `DataContractSpecification` and it is published by
//! `datacontract/datacontract-specification`. `specs.toml` records that
//! discrepancy in full rather than smoothing it over, and keeps `id = "odcl"`
//! so existing references still resolve. [`SPEC_ID`] is therefore `odcl`, and
//! so is the prefix on every code in [`codes`] — a code prefix is matched on
//! by tooling and must not drift from the registry id it accompanies.
//!
//! # Why this exists
//!
//! ODCL documents in this estate were already being validated. The function
//! doing it looks like this:
//!
//! ```text
//! pub fn validate_odcl_internal(content: &str) -> Result<(), String>
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
//! threw away.
//!
//! # The verdict is deliberately unchanged
//!
//! Errors here are **exactly** the published schema's findings, plus two
//! intake failures (the document does not parse; the document is empty).
//! Everything this crate adds beyond the schema — see [`rules`] — is a warning
//! or an info, and the default [`GatePolicy`](conform_core::GatePolicy) gates
//! on errors alone. So this validator passes and fails the same documents the
//! schema does, by construction.
//!
//! That is not a limitation; it is the property that makes the change safe to
//! adopt. `tests/oracle_agreement.rs` holds this crate's verdict against the
//! pre-existing `validate_odcl_internal`'s over a fixture corpus and fails on
//! any disagreement that is not recorded, explained and proven to still
//! diverge.
//!
//! # What it adds that a schema cannot say
//!
//! JSON Schema records plenty it cannot enforce, and this standard's schema
//! records more than most:
//!
//! - the **root object is open** — unlike ODCS, ODCL does not close
//!   `additionalProperties` at the root, so `modles:` is accepted in silence
//!   and every rule written against `models` simply never fires. Reported as
//!   [`codes::UNKNOWN_ROOT_KEY`];
//! - five keys carry a **`deprecationMessage`**, an annotation no validator
//!   asserts on. Reported as [`codes::DEPRECATED_KEYWORD`], quoting upstream's
//!   own wording;
//! - `info.status` publishes its values as **`examples`** rather than an
//!   `enum`. Reported as [`codes::UNCONVENTIONAL_STATUS`];
//! - `references` and `$ref` are typed as bare strings, so the schema says
//!   nothing about where they point. See below.
//!
//! Every one of these is read out of the vendored schema at construction
//! rather than transcribed here, so none of them can drift from the document
//! they describe.
//!
//! # References resolve three ways, not two
//!
//! A field's `references` names another model's field; a field's `$ref` names
//! a definition "internally or externally", in the schema's own words. So
//! following one has three possible answers, and [`resolve_field_reference`]
//! and [`resolve_ref`] return [`Resolution`](conform_core::Resolution) rather
//! than an `Option` to keep them apart: found, genuinely absent, or **never
//! looked at**. An external `$ref` is the third — this crate reads one
//! document and performs no network access — and reporting it as a broken
//! reference would be a false alarm. A gate that raises false alarms gets
//! switched off, after which it protects nothing at all.
//!
//! # This crate defines no vocabulary of its own
//!
//! Every diagnostic, severity, location, report, gate and resolution here is
//! `conform-core`'s. There is no local `Finding`, no local `Report`, no local
//! `Severity`. That is not a style preference — it is what makes an ODCL
//! failure render through the identical path as an ODCS, ODPS or OKF failure —
//! and it is enforced by `tests/no_bespoke_vocabulary.rs`, which reads these
//! sources.
//!
//! # Messages quote the document, and this crate does not escape them
//!
//! A diagnostic's `message` interpolates values the document's author wrote —
//! a key, a status, a reference string — because a finding that does not quote
//! what it found is not actionable. Those values are **not** sanitised here. A
//! document can therefore carry a bidirectional override or a zero-width run
//! into a message, and a renderer that writes it straight to a terminal will
//! show a line the document rewrote.
//!
//! That is the renderer's boundary, not this crate's, and the split is
//! deliberate: escaping is not idempotent — a `\` doubles on every pass — so
//! it has to happen exactly once, at the point of display. `conform-cli` is
//! where it happens in this repository, and `conform-okf` documents the same
//! convention.
//!
//! # Example
//!
//! ```no_run
//! use conform_core::{GatePolicy, Severity, Validator};
//! use conform_lexicon::{Document, LexiconValidator};
//!
//! let validator = LexiconValidator::from_registry_path("specs.toml")?;
//!
//! let report = validator.validate(&Document::new(
//!     "contracts/orders.yaml",
//!     "dataContractSpecification: 1.2.1\nid: orders\n",
//! ));
//!
//! // `info` is required and absent, and `info.title`/`info.version` with it —
//! // each its own finding, which is the thing `Result<(), String>` has no way
//! // to express.
//! for diagnostic in report.at_or_above(Severity::Error) {
//!     println!("{diagnostic}");
//! }
//! assert!(report.should_gate(GatePolicy::default()));
//! # Ok::<(), conform_lexicon::SchemaError>(())
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

mod document;
mod references;
mod rules;
mod schema;
mod validator;

pub use document::Document;
pub use references::{resolve_field_reference, resolve_ref};
pub use validator::{LexiconValidator, SPEC_ID, SchemaError};
