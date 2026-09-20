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
//! | Band | Meaning | Severity |
//! |---|---|---|
//! | `ODCL0xx` | intake — the bytes could not be turned into a document at all | error |
//! | `ODCL1xx` | schema conformance — the document violates the published schema | error |
//! | `ODCL2xx` | hygiene — the document conforms, but something about it is questionable | warning |
//! | `ODCL3xx` | cross-reference — what happened when a reference inside the document was followed | warning where a target is absent, info where it resolved or was not followed |
//! | `ODCL9xx` | setup and provenance — about the validator, not the document | error, except [`VALIDATED_AGAINST`] |
//!
//! Only the `ODCL0xx` and `ODCL1xx` bands are errors about the document.
//! Everything this crate adds beyond the published schema is a warning or a
//! note, so a document this crate flags still passes the default gate — which
//! is what lets these rules exist without changing anybody's pass/fail
//! verdict. `tests/oracle_agreement.rs` holds that claim to account against
//! the pre-existing validator.
//!
//! Three codes are information rather than a complaint, and all three are the
//! same shape: they say *something was checked*, or *deliberately was not*,
//! rather than *something is wrong*. [`VALIDATED_AGAINST`] says the schema's
//! bytes were verified; [`REFERENCE_RESOLVED`] says a reference was followed
//! and found; [`REFERENCE_NOT_INSPECTED`] says one was not followed and why.
//! `tests/codes_are_a_contract.rs` spells all three out, so the exceptions
//! cannot spread quietly.
//!
//! That the two "nothing is wrong" cases are reported *at all* is the point.
//! A validator silent about a reference it followed successfully and one
//! silent about a reference it never followed look identical from the outside,
//! and that indistinguishability is the defect this family of crates exists to
//! remove.
//!
//! # Why the prefix is `ODCL` and the crate is `conform-lexicon`
//!
//! `specs.toml` records the tension in full: this estate calls the standard
//! ODCL, "Open Data Contract Language", while the document's own `title` is
//! `DataContractSpecification`. The registry entry's `id` is `odcl`, so that
//! is what [`SPEC_ID`](crate::SPEC_ID) resolves and that is what these codes
//! are namespaced to. A code prefix is matched on by tooling and must not
//! drift from the registry id it accompanies.

// ---------------------------------------------------------------------------
// ODCL0xx — intake.
// ---------------------------------------------------------------------------

/// The document is not well-formed YAML or JSON, so there is nothing to check
/// it against. Carries the parser's own line and column.
pub const UNPARSEABLE: &str = "ODCL001";

/// The document parsed to nothing at all — an empty file, or one holding only
/// comments. Distinguished from [`UNPARSEABLE`] because "you gave me an empty
/// file" and "you gave me broken YAML" are different mistakes with different
/// fixes.
pub const EMPTY: &str = "ODCL002";

// ---------------------------------------------------------------------------
// ODCL1xx — schema conformance. One code per class of JSON Schema keyword,
// because "the document is wrong" is not an actionable statement and
// "`info.version` is required and absent" is.
// ---------------------------------------------------------------------------

/// A schema violation this crate has no more specific code for.
///
/// The catch-all exists so that a new keyword class appearing in a future
/// version of the specification — or in a future version of the underlying
/// JSON Schema implementation — is *reported* rather than silently dropped. A
/// finding under this code is a hint that a more specific code should be
/// added.
pub const SCHEMA_VIOLATION: &str = "ODCL100";

/// A required property is absent. At the root that is one of
/// `dataContractSpecification`, `id` or `info`; inside `info` it is `title` or
/// `version`.
pub const REQUIRED_MISSING: &str = "ODCL101";

/// A property is present that the schema does not permit here.
///
/// Note where this *cannot* fire: the ODCL root schema does not close
/// `additionalProperties`, so an unrecognised top-level key is accepted by the
/// published schema. That gap is covered by the hygiene rule
/// [`UNKNOWN_ROOT_KEY`] instead, as a warning, because the schema is the
/// authority on what conforms.
pub const UNKNOWN_PROPERTY: &str = "ODCL102";

/// A value is outside the set the schema permits — `enum` or `const`. The
/// commonest case by far is a field `type` outside the specification's
/// twenty-seven logical types.
pub const VALUE_NOT_PERMITTED: &str = "ODCL103";

/// A value is of the wrong JSON type.
pub const WRONG_TYPE: &str = "ODCL104";

/// A value fails a `pattern`, `propertyNames`, `format`, `contentEncoding` or
/// `contentMediaType` constraint.
pub const MALFORMED_VALUE: &str = "ODCL105";

/// A collection has the wrong number of members, or repeats one it may not —
/// `minItems`, `maxItems`, `minProperties`, `maxProperties`, `uniqueItems`,
/// `contains`.
pub const BAD_CARDINALITY: &str = "ODCL106";

/// A number or string is outside its permitted range — `minimum`, `maximum`,
/// their exclusive forms, `multipleOf`, `minLength`, `maxLength`.
pub const OUT_OF_RANGE: &str = "ODCL107";

/// A value matches none of the permitted alternatives, or matches more than
/// one where exactly one was required — `oneOf`, `anyOf`, `not`, and a schema
/// position that permits nothing at all. The `servers` map dispatches on
/// `type` through a long `allOf`/`if`/`then` chain, so a misconfigured server
/// commonly lands here.
pub const NO_MATCHING_ALTERNATIVE: &str = "ODCL108";

// ---------------------------------------------------------------------------
// ODCL2xx — hygiene. Warnings, never errors: every one of these describes a
// document the published schema accepts.
// ---------------------------------------------------------------------------

/// The document declares a `dataContractSpecification` version other than the
/// one this validator carries. The schema's `enum` permits seven versions, so
/// this is legal; it is still worth saying, because a rule written for a newer
/// version cannot have been applied to an older document.
pub const SPEC_VERSION_BEHIND: &str = "ODCL200";

/// The contract catalogues no models, so it describes a dataset without
/// describing its shape. `models` is optional at the root — a contract with no
/// `models` key at all conforms — which is exactly why a rule here earns its
/// place.
pub const NO_MODELS: &str = "ODCL201";

/// The contract names no server, so nothing in it says where the data is.
pub const NO_SERVERS: &str = "ODCL202";

/// The contract names no owner, so there is nobody to ask about it.
pub const NO_OWNER: &str = "ODCL203";

/// The contract carries no description, so its purpose lives only in whatever
/// its `id` and `title` suggest.
pub const NO_DESCRIPTION: &str = "ODCL204";

/// `info.status` holds a value outside the conventional set. The schema
/// publishes that set as `examples` rather than as an `enum`, so it cannot
/// enforce it — which is exactly why a rule here earns its place.
pub const UNCONVENTIONAL_STATUS: &str = "ODCL205";

/// A top-level key the published schema does not name.
///
/// The ODCL root object does **not** close `additionalProperties`, so a
/// misspelled `modles:` or `servicelevel:` is accepted in silence and every
/// rule written against the real key simply never fires. ODCS closes both
/// `additionalProperties` and `unevaluatedProperties` at its root and so
/// catches this as an error; ODCL cannot, and the difference is invisible to
/// anyone reading only the verdict.
///
/// The permitted set is read from the vendored schema's own `properties` at
/// construction, never transcribed here, so this rule cannot drift from the
/// document it is derived from.
pub const UNKNOWN_ROOT_KEY: &str = "ODCL206";

/// A field declares neither a `type` nor a `$ref`, so nothing in the contract
/// says what the data in it looks like. Both keys are optional in the schema,
/// and a field carrying only a description is a field no consumer can check
/// anything against.
pub const UNTYPED_FIELD: &str = "ODCL207";

/// A key the specification marks deprecated is in use.
///
/// The schema records these with a `deprecationMessage` annotation, which is
/// not a JSON Schema assertion keyword: a validator reads straight past it and
/// the document passes. The messages are carried verbatim from the schema
/// rather than paraphrased, so this rule says what upstream says.
pub const DEPRECATED_KEYWORD: &str = "ODCL208";

// ---------------------------------------------------------------------------
// ODCL3xx — cross-reference. What happened when a reference *inside* the
// document was followed, expressed as `conform_core::Resolution` and reported
// with the two non-resolving outcomes kept apart.
// ---------------------------------------------------------------------------

/// A field's `references` names a `model.field` that this document does not
/// contain.
///
/// The document set *was* inspected and the target is genuinely absent — a
/// [`Resolution::DoesNotExist`](conform_core::Resolution::DoesNotExist). A
/// warning rather than an error because the published schema types
/// `references` as a plain string and says nothing about where it must point;
/// errors here are the schema's findings alone.
pub const REFERENCE_DOES_NOT_EXIST: &str = "ODCL300";

/// A field's `$ref` names `#/definitions/…` inside this document, and this
/// document has no such definition.
///
/// As [`REFERENCE_DOES_NOT_EXIST`]: the target was looked for and is absent.
pub const DEFINITION_DOES_NOT_EXIST: &str = "ODCL301";

/// A reference was **not followed**, and this says so rather than guessing.
///
/// An external `$ref` — `https://…`, or another file — names something outside
/// the document under test, and this crate performs no network access and
/// loads no second document. The honest answer is that nobody looked, so this
/// is a [`Resolution::NotInspected`](conform_core::Resolution::NotInspected)
/// reported as **information**, never as a missing target.
///
/// Paired with [`REFERENCE_RESOLVED`], which is what makes it meaningful: a
/// reader can tell the references this run actually followed from the ones it
/// declined to, without inferring either from silence.
pub const REFERENCE_NOT_INSPECTED: &str = "ODCL302";

/// A reference was followed and its target found in this document.
///
/// A [`Resolution::Resolved`](conform_core::Resolution::Resolved), reported as
/// information and naming the pointer it landed on. Nothing is wrong; what is
/// recorded is that the check *ran*, which is the only thing that distinguishes
/// it from [`REFERENCE_NOT_INSPECTED`].
pub const REFERENCE_RESOLVED: &str = "ODCL303";

// ---------------------------------------------------------------------------
// ODCL9xx — the validator's own setup. These describe this repository's
// configuration, not the document under test.
// ---------------------------------------------------------------------------

/// The registry holds no entry for this standard, so there is no schema to
/// validate against and no provenance to check.
pub const NOT_IN_REGISTRY: &str = "ODCL900";

/// The vendored schema could not be read from where the registry says it is.
pub const SCHEMA_UNREADABLE: &str = "ODCL901";

/// The vendored schema is not valid JSON, or is not a JSON Schema this crate
/// can compile.
pub const SCHEMA_UNUSABLE: &str = "ODCL902";

/// The vendored schema's bytes no longer match the digest the registry records
/// for them. Validating against it anyway would produce verdicts nobody could
/// trace to a published standard, so this crate refuses.
pub const SCHEMA_PROVENANCE_FAILED: &str = "ODCL903";

/// Which schema this run validated against, and that its bytes were checked.
/// Reported as information on every report, because "validated against the
/// pinned 1.2.1 schema" and "validated against something" look identical
/// otherwise.
pub const VALIDATED_AGAINST: &str = "ODCL904";
