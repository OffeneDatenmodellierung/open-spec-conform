//! The validator: where the schema comes from, and what it produces.

use std::fmt;
use std::path::Path;

use conform_core::{
    ConformanceReport, Diagnostic, DocumentId, Location, Severity, SpecRef, Validator,
};
use conform_registry::Registry;
use serde_json::Value;

use crate::codes;
use crate::document::Document;
use crate::rules;
use crate::schema;

/// The identifier this crate looks for in `specs.toml`.
///
/// Part of the public contract: a registry that renames this entry is a
/// registry this crate cannot find its schema in, and it will say so under
/// [`codes::NOT_IN_REGISTRY`] rather than fall back to a guess.
pub const SPEC_ID: &str = "odcs";

/// Validates Open Data Contract Standard documents against the schema the
/// registry pins.
///
/// # Where the schema comes from
///
/// [`from_registry`](Self::from_registry) is the constructor that matters. It
/// does three things in order, and refuses at the first that fails:
///
/// 1. finds the `odcs` entry in the registry — a schema this repository has
///    no provenance record for is a schema this crate will not use;
/// 2. **re-hashes the vendored bytes** against the digest the registry
///    records, via [`Registry::verify_entry`];
/// 3. compiles what it read.
///
/// Step 2 is the one worth arguing about, because it makes construction do
/// filesystem work that a naive `include_str!` would avoid. It is there
/// because the alternative is a validator that will happily issue confident
/// verdicts against an edited schema, and a verdict nobody can trace to a
/// published standard is not a conformance verdict at all. The ODPS schema in
/// this estate was vendored as `…-latest.json` with no version, no source URL
/// and no fetch date; this is the check that stops that happening quietly a
/// second time.
///
/// # What it produces
///
/// A [`ConformanceReport`], never a `Result`. Every fault in the document is a
/// separate [`Diagnostic`] with a stable code, a severity and a JSON Pointer:
///
/// - **errors** are exactly the published schema's findings, plus the two
///   intake failures (unparseable, empty). Nothing else is ever an error, so
///   this crate's pass/fail verdict is the schema's verdict.
/// - **warnings** are the hygiene rules in [`crate::rules`], which the schema
///   permits and a reviewer would not.
/// - one **info** note per report says which schema was used and that its
///   bytes were verified.
pub struct OdcsValidator {
    schema: jsonschema::Validator,
    spec: SpecRef,
    provenance: String,
}

impl OdcsValidator {
    /// Build a validator from a loaded registry.
    ///
    /// # Errors
    ///
    /// Returns a [`SchemaError`] — itself carrying diagnostics, not a string —
    /// if the registry has no `odcs` entry, if the vendored schema is missing,
    /// unreadable, or no longer hashes to its recorded digest, or if it cannot
    /// be compiled as a JSON Schema.
    pub fn from_registry(registry: &Registry) -> Result<Self, SchemaError> {
        let source = Location::document(registry.source().clone());

        let Some(index) = registry.entries().iter().position(|e| e.id == SPEC_ID) else {
            return Err(SchemaError::single(
                Diagnostic::error(
                    codes::NOT_IN_REGISTRY,
                    source,
                    format!("the registry holds no `{SPEC_ID}` entry, so there is no schema to validate against"),
                )
                .with_help(
                    "add the entry to `specs.toml`, with the vendored schema's path, digest and \
                     upstream pin",
                ),
            ));
        };
        let entry = &registry.entries()[index];

        // Provenance before use. `verify_entry` re-hashes the bytes on disk;
        // its own diagnostic is kept verbatim so the reader gets the registry's
        // `REG0xx` code and its explanation, not a paraphrase.
        let integrity = registry.verify_entry(index, entry);
        if integrity.severity >= Severity::Error {
            let mut report = ConformanceReport::new();
            report.push(Diagnostic::error(
                codes::SCHEMA_PROVENANCE_FAILED,
                integrity.location.clone(),
                format!(
                    "refusing to validate against the `{SPEC_ID}` schema: its provenance check failed"
                ),
            )
            .with_help(
                "a verdict issued against a schema nobody can trace to a published standard is \
                 not a conformance verdict — fix the artefact or the registry, do not bypass this",
            ));
            report.push(integrity);
            return Err(SchemaError { report });
        }

        let path = registry.artefact_path(entry);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            SchemaError::single(Diagnostic::error(
                codes::SCHEMA_UNREADABLE,
                Location::document(path.display().to_string()),
                format!("cannot read the vendored `{SPEC_ID}` schema: {error}"),
            ))
        })?;

        let mut spec = SpecRef::new(entry.id.clone());
        if let Some(version) = &entry.version {
            spec = spec.with_version(version.clone());
        }

        let provenance = match &entry.pinned_ref {
            Some(pinned) => format!(
                "bytes at `{}` verified against the digest `specs.toml` records for upstream pin \
                 `{pinned}`",
                entry.vendored_path
            ),
            None => format!(
                "bytes at `{}` verified against the digest `specs.toml` records; the entry \
                 records no upstream pin",
                entry.vendored_path
            ),
        };

        Self::from_schema_str(&text, spec, provenance, &path.display().to_string())
    }

    /// Load a registry from a path, then build a validator from it.
    ///
    /// # Errors
    ///
    /// As [`from_registry`](Self::from_registry), and additionally if the
    /// registry file itself cannot be read or is malformed — in which case the
    /// [`SchemaError`] carries `conform-registry`'s own diagnostics unchanged.
    pub fn from_registry_path(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let registry = Registry::load_path(path).map_err(|error| SchemaError {
            report: error.into_report(),
        })?;
        Self::from_registry(&registry)
    }

    /// Build a validator from schema text already in hand, bypassing the
    /// registry.
    ///
    /// The escape hatch, and named like one. `provenance` is the sentence that
    /// will appear on every report under [`codes::VALIDATED_AGAINST`]; a
    /// caller using this constructor is taking responsibility for what that
    /// sentence claims, because nothing here checks it.
    ///
    /// # Errors
    ///
    /// Returns a [`SchemaError`] if the text is not JSON, or is not a JSON
    /// Schema that can be compiled.
    pub fn from_schema_str(
        schema_json: &str,
        spec: SpecRef,
        provenance: impl Into<String>,
        schema_name: &str,
    ) -> Result<Self, SchemaError> {
        let document = Location::document(schema_name.to_owned());

        let value: Value = serde_json::from_str(schema_json).map_err(|error| {
            SchemaError::single(Diagnostic::error(
                codes::SCHEMA_UNUSABLE,
                document
                    .clone()
                    .with_line(truncate(error.line()))
                    .with_column(truncate(error.column())),
                format!("the vendored `{SPEC_ID}` schema is not valid JSON: {error}"),
            ))
        })?;

        let schema = schema::compile(&value).map_err(|error| {
            SchemaError::single(Diagnostic::error(
                codes::SCHEMA_UNUSABLE,
                document.with_pointer(error.schema_path().to_string()),
                format!("the vendored `{SPEC_ID}` schema does not compile: {error}"),
            ))
        })?;

        Ok(Self {
            schema,
            spec,
            provenance: provenance.into(),
        })
    }

    /// The sentence this validator reports its schema's provenance with.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Validate a document given by name and text, without constructing a
    /// [`Document`] first.
    #[must_use]
    pub fn validate_text(&self, id: impl Into<DocumentId>, text: &str) -> ConformanceReport {
        self.validate(&Document::new(id, text))
    }
}

impl Validator for OdcsValidator {
    type Document = Document;

    fn spec(&self) -> SpecRef {
        self.spec.clone()
    }

    fn validate(&self, document: &Document) -> ConformanceReport {
        let id = document.id();
        let mut report = ConformanceReport::new();
        report.push(schema::validated_against(id, &self.spec, &self.provenance));

        let instance = match schema::parse(document.text(), id, &self.spec) {
            Ok(instance) => instance,
            Err(diagnostic) => {
                // Nothing downstream of a failed parse can say anything true
                // about this document, so this is the one place the report
                // stops early.
                report.push(*diagnostic);
                return report;
            }
        };

        report.extend(schema::violations(&self.schema, &instance, id, &self.spec));
        report.extend(rules::hygiene(&instance, id, &self.spec));
        report
    }
}

// `jsonschema::Validator` is not `Debug`, and the workspace requires every
// public type to be. Printing a compiled schema would be noise anyway; what a
// reader of a debug dump wants is which standard this is and where its schema
// came from.
impl fmt::Debug for OdcsValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OdcsValidator")
            .field("spec", &self.spec)
            .field("provenance", &self.provenance)
            .field("schema", &"<compiled JSON Schema>")
            .finish()
    }
}

/// A validator that could not be built.
///
/// Carries a [`ConformanceReport`] rather than a string, for the same reason
/// everything else in this family does: a caller renders a setup failure
/// through exactly the same path as a conformance failure, with a code and a
/// location, instead of matching on prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    report: ConformanceReport,
}

impl SchemaError {
    fn single(diagnostic: Diagnostic) -> Self {
        let mut report = ConformanceReport::new();
        report.push(diagnostic);
        Self { report }
    }

    /// The diagnostics explaining the failure.
    #[must_use]
    pub const fn report(&self) -> &ConformanceReport {
        &self.report
    }

    /// Take the diagnostics, to merge into a larger report.
    #[must_use]
    pub fn into_report(self) -> ConformanceReport {
        self.report
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.report.diagnostics() {
            [] => f.write_str("the validator could not be built"),
            [first, rest @ ..] => {
                write!(f, "{first}")?;
                if !rest.is_empty() {
                    write!(f, " (and {} more)", rest.len())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SchemaError {}

/// Saturating `usize` → `u32`, matching `conform-core`'s width for a line
/// number.
fn truncate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
