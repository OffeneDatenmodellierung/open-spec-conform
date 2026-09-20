//! The stable codes this crate raises findings under.
//!
//! Stability is the contract `conform-core` asks for, and it starts now:
//! downstream tooling, suppression lists and CI annotations match on these
//! strings, so a code is **retired rather than reused**, and the message
//! attached to one may be reworded freely while the code may not. Changing
//! which code a rule raises is a major version of this crate.
//!
//! The number space is banded so a reader can tell what *kind* of thing went
//! wrong from the code alone, before reading the message:
//!
//! | Band | Meaning |
//! |---|---|
//! | `ODPS0xx` | intake — the bytes could not be turned into a document at all |
//! | `ODPS1xx` | schema conformance — the document violates the published schema |
//! | `ODPS2xx` | hygiene — the document conforms, but something about it is questionable |
//! | `ODPS9xx` | setup and provenance — about the validator, not the document |
//!
//! The bands line up deliberately with `conform-odcs`'s: a reader who has
//! learned one number space has learned both, and tooling that groups findings
//! by band works across the two without a table. What is **not** shared is the
//! codes themselves — `ODCS101` and `ODPS101` are different strings naming
//! findings against different standards, and nothing may collapse them.

// ---------------------------------------------------------------------------
// ODPS0xx — intake.
// ---------------------------------------------------------------------------

/// The document is not well-formed YAML or JSON, so there is nothing to check
/// it against. Carries the parser's own line and column.
pub const UNPARSEABLE: &str = "ODPS001";

/// The document parsed to nothing at all — an empty file, or one holding only
/// comments. Distinguished from [`UNPARSEABLE`] because "you gave me an empty
/// file" and "you gave me broken YAML" are different mistakes with different
/// fixes.
pub const EMPTY: &str = "ODPS002";

// ---------------------------------------------------------------------------
// ODPS1xx — schema conformance. One code per class of JSON Schema keyword,
// because "the document is wrong" is not an actionable statement and
// "`status` is required and absent" is.
// ---------------------------------------------------------------------------

/// A schema violation this crate has no more specific code for.
///
/// The catch-all exists so that a new keyword class appearing in a future ODPS
/// schema — or in a future version of the underlying JSON Schema
/// implementation — is *reported* rather than silently dropped. A finding
/// under this code is a hint that a more specific code should be added.
pub const SCHEMA_VIOLATION: &str = "ODPS100";

/// A required property is absent.
pub const REQUIRED_MISSING: &str = "ODPS101";

/// A property is present that the schema does not permit here. ODPS closes
/// `additionalProperties` at the root and on every port definition, so a
/// misspelled key lands here rather than being silently ignored.
pub const UNKNOWN_PROPERTY: &str = "ODPS102";

/// A value is outside the set the schema permits — `enum` or `const`.
pub const VALUE_NOT_PERMITTED: &str = "ODPS103";

/// A value is of the wrong JSON type.
pub const WRONG_TYPE: &str = "ODPS104";

/// A value fails a `pattern`, `format`, `contentEncoding` or
/// `contentMediaType` constraint.
pub const MALFORMED_VALUE: &str = "ODPS105";

/// A collection has the wrong number of members, or repeats one it may not —
/// `minItems`, `maxItems`, `minProperties`, `maxProperties`, `uniqueItems`,
/// `contains`.
pub const BAD_CARDINALITY: &str = "ODPS106";

/// A number or string is outside its permitted range — `minimum`, `maximum`,
/// their exclusive forms, `multipleOf`, `minLength`, `maxLength`.
pub const OUT_OF_RANGE: &str = "ODPS107";

/// A value matches none of the permitted alternatives, or matches more than
/// one where exactly one was required — `oneOf`, `anyOf`, `not`, and a schema
/// position that permits nothing at all.
pub const NO_MATCHING_ALTERNATIVE: &str = "ODPS108";

// ---------------------------------------------------------------------------
// ODPS2xx — hygiene. Warnings, never errors: every one of these describes a
// document the published schema accepts.
// ---------------------------------------------------------------------------

/// The document declares an `apiVersion` other than the one this validator
/// carries. The schema permits a range of versions, so this is legal; it is
/// still worth saying, because a rule written for a newer version cannot have
/// been applied to an older document.
pub const API_VERSION_BEHIND: &str = "ODPS200";

/// The product declares no output port.
///
/// The schema's own description of `outputPorts` reads "You need at least one,
/// as a data product without output is useless" — and then does not require
/// one. This rule is that sentence, enforced at the severity the schema's
/// silence permits.
pub const NO_OUTPUT_PORTS: &str = "ODPS201";

/// The product declares no input port. The schema says "You need at least one
/// as a data product needs to get data somewhere", and likewise does not
/// require it.
pub const NO_INPUT_PORTS: &str = "ODPS202";

/// The product names no owning team, so there is nobody to ask about it.
pub const NO_TEAM: &str = "ODPS203";

/// The product carries no description, so its purpose lives only in whatever
/// its `id` suggests.
pub const NO_DESCRIPTION: &str = "ODPS204";

/// `status` holds a value outside the conventional set. The schema publishes
/// that set as `examples` rather than as an `enum`, so it cannot enforce it —
/// which is exactly why a rule here earns its place.
pub const UNCONVENTIONAL_STATUS: &str = "ODPS205";

/// The product records no `version`. The schema's own wording is "Not
/// required, but highly recommended", which is a warning in prose; this is the
/// same warning in a form tooling can read.
pub const NO_VERSION: &str = "ODPS206";

// ---------------------------------------------------------------------------
// ODPS9xx — the validator's own setup. These describe this repository's
// configuration, not the document under test.
// ---------------------------------------------------------------------------

/// The registry holds no entry for this standard, so there is no schema to
/// validate against and no provenance to check.
pub const NOT_IN_REGISTRY: &str = "ODPS900";

/// The vendored schema could not be read from where the registry says it is.
pub const SCHEMA_UNREADABLE: &str = "ODPS901";

/// The vendored schema is not valid JSON, or is not a JSON Schema this crate
/// can compile.
pub const SCHEMA_UNUSABLE: &str = "ODPS902";

/// The vendored schema's bytes no longer match the digest the registry records
/// for them. Validating against it anyway would produce verdicts nobody could
/// trace to a published standard, so this crate refuses.
///
/// This is the check that matters most in this crate specifically: the ODPS
/// schema is the artefact that was vendored as `odps-json-schema-latest.json`
/// with no version, no source URL, no fetch date and no `$id`, and is the
/// defect `conform-registry` was written for.
pub const SCHEMA_PROVENANCE_FAILED: &str = "ODPS903";

/// Which schema this run validated against, and that its bytes were checked.
/// Reported as information on every report, because "validated against the
/// pinned v1.0.0 schema" and "validated against something" look identical
/// otherwise — and that indistinguishability is the defect this whole family
/// of crates exists to remove.
pub const VALIDATED_AGAINST: &str = "ODPS904";
