//! Compiling the vendored schema, and turning what it says into diagnostics.
//!
//! The pre-existing validator in `data-modelling-sdk` compiles this same schema
//! and then throws away everything but the first failure:
//!
//! ```text
//! pub fn validate_odcl_internal(content: &str) -> Result<(), String>
//! ```
//!
//! One `String`. No severity, no stable code, no location, and — because
//! `jsonschema`'s `validate` short-circuits — no second finding, however many
//! the document has. This module is the recovery of that detail: every
//! violation the schema finds becomes its own [`Diagnostic`], coded by the
//! *class* of constraint that failed and located by the JSON Pointer of the
//! value that failed it.

use conform_core::{Diagnostic, Location, Severity, SpecRef};
use jsonschema::error::ValidationErrorKind;
use serde_json::Value;

use crate::codes;

/// Compile a JSON Schema document.
///
/// Draft detection is left to `jsonschema`, which reads the schema's own
/// `$schema` keyword. The vendored ODCL schema declares **draft-07**, where
/// the two Bitol schemas this family also validates declare 2019-09;
/// hard-coding a draft here would mean a schema that moved draft was validated
/// under the wrong rules while still looking green.
///
/// `should_validate_formats(true)` is set explicitly rather than left to the
/// default. Under draft-07 `format` is an assertion and the default already
/// checks it — which is why the pre-existing validator, built with a plain
/// `jsonschema::Validator::new`, checks it too and why setting this changes no
/// verdict. Stating it is what keeps that true if the vendored schema ever
/// moves to 2019-09 or later, where `format` silently degrades to an
/// annotation and `info.contact.email` would stop being checked at all.
pub(crate) fn compile(
    schema: &Value,
) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>> {
    jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(jsonschema::ValidationError::to_owned)
}

/// Every way this document violates the schema, one diagnostic each.
///
/// Order is `jsonschema`'s own traversal order, which is stable for a given
/// schema and instance. Nothing here deduplicates or truncates: a document
/// with forty faults reports forty findings, because the caller — a CI
/// annotation pass, an editor, a human reading a list — is the only party that
/// knows how many it wants to see.
pub(crate) fn violations(
    validator: &jsonschema::Validator,
    instance: &Value,
    document: &conform_core::DocumentId,
    spec: &SpecRef,
) -> Vec<Diagnostic> {
    validator
        .iter_errors(instance)
        .map(|error| {
            let location = at(document, &error.instance_path().to_string());
            // The schema path is the clause of the standard that objected, so
            // it belongs in the spec reference rather than in the message —
            // that is what `SpecRef::section` is for, and it is what lets a
            // reader go from a finding to the schema text that produced it.
            let spec_ref = spec.clone().with_section(error.schema_path().to_string());
            Diagnostic::error(code_for(error.kind()), location, error.to_string())
                .with_spec_ref(spec_ref)
        })
        .collect()
}

/// A location inside the document under test, from a JSON Pointer.
///
/// The empty pointer is the document root; JSON Pointer spells that as the
/// empty string, and an empty `(…)` in a rendered location reads as a bug. A
/// root-level finding therefore names the document and nothing finer, which is
/// exactly what [`Location::document`] means.
pub(crate) fn at(document: &conform_core::DocumentId, pointer: &str) -> Location {
    let location = Location::document(document.clone());
    if pointer.is_empty() {
        location
    } else {
        location.with_pointer(pointer)
    }
}

/// Which stable code names this class of constraint failure.
///
/// Matched by keyword class rather than by keyword, so that `minLength` and
/// `maximum` — both "this value is outside its permitted range" — do not need a
/// code each, while `required` and `enum`, which a reader acts on completely
/// differently, do.
///
/// The wildcard arm is load-bearing: [`ValidationErrorKind`] is not
/// `#[non_exhaustive]` today, but a new variant in a future minor release of
/// `jsonschema` must degrade to [`codes::SCHEMA_VIOLATION`] and stay reported,
/// never vanish.
fn code_for(kind: &ValidationErrorKind) -> &'static str {
    match kind {
        ValidationErrorKind::Required { .. } => codes::REQUIRED_MISSING,

        ValidationErrorKind::AdditionalProperties { .. }
        | ValidationErrorKind::UnevaluatedProperties { .. }
        | ValidationErrorKind::AdditionalItems { .. }
        | ValidationErrorKind::UnevaluatedItems { .. } => codes::UNKNOWN_PROPERTY,

        ValidationErrorKind::Enum { .. } | ValidationErrorKind::Constant { .. } => {
            codes::VALUE_NOT_PERMITTED
        }

        ValidationErrorKind::Type { .. } => codes::WRONG_TYPE,

        // `propertyNames` sits here rather than with the unknown-property
        // class above, because in this schema it is a `pattern`: `models` and
        // `definitions` constrain the *spelling* of their keys, so a violation
        // means the name is malformed, not that the key is unrecognised.
        ValidationErrorKind::PropertyNames { .. }
        | ValidationErrorKind::Pattern { .. }
        | ValidationErrorKind::Format { .. }
        | ValidationErrorKind::ContentEncoding { .. }
        | ValidationErrorKind::ContentMediaType { .. } => codes::MALFORMED_VALUE,

        ValidationErrorKind::MinItems { .. }
        | ValidationErrorKind::MaxItems { .. }
        | ValidationErrorKind::MinProperties { .. }
        | ValidationErrorKind::MaxProperties { .. }
        | ValidationErrorKind::UniqueItems
        | ValidationErrorKind::Contains => codes::BAD_CARDINALITY,

        ValidationErrorKind::Minimum { .. }
        | ValidationErrorKind::Maximum { .. }
        | ValidationErrorKind::ExclusiveMinimum { .. }
        | ValidationErrorKind::ExclusiveMaximum { .. }
        | ValidationErrorKind::MultipleOf { .. }
        | ValidationErrorKind::MinLength { .. }
        | ValidationErrorKind::MaxLength { .. } => codes::OUT_OF_RANGE,

        ValidationErrorKind::AnyOf { .. }
        | ValidationErrorKind::OneOfNotValid { .. }
        | ValidationErrorKind::OneOfMultipleValid { .. }
        | ValidationErrorKind::Not { .. }
        | ValidationErrorKind::FalseSchema => codes::NO_MATCHING_ALTERNATIVE,

        _ => codes::SCHEMA_VIOLATION,
    }
}

/// Parse a document's text as YAML, accepting JSON as the subset of YAML it is.
///
/// Returns the parsed value, or the single diagnostic explaining why there
/// isn't one. A parse failure carries the parser's own line and column, which
/// is the one place in this crate where a finding can be that precise — see
/// the crate documentation on why schema violations cannot be.
pub(crate) fn parse(
    text: &str,
    document: &conform_core::DocumentId,
    spec: &SpecRef,
) -> Result<Value, Box<Diagnostic>> {
    match serde_norway::from_str::<Value>(text) {
        Ok(Value::Null) => Err(Box::new(
            Diagnostic::error(
                codes::EMPTY,
                Location::document(document.clone()),
                "document is empty: there is no data contract here to check",
            )
            .with_help(
                "a data contract needs at least `dataContractSpecification`, `id` and an `info` \
                 carrying `title` and `version`",
            )
            .with_spec_ref(spec.clone()),
        )),
        Ok(value) => Ok(value),
        Err(error) => {
            let mut location = Location::document(document.clone());
            if let Some(mark) = error.location() {
                // `serde_norway` reports one-based line and column, which is
                // what `Location` wants; no adjustment, and none should creep
                // in later.
                location = location
                    .with_line(truncate(mark.line()))
                    .with_column(truncate(mark.column()));
            }
            Err(Box::new(
                Diagnostic::error(codes::UNPARSEABLE, location, error.to_string())
                    .with_help("fix the syntax error; nothing else can be checked until the document parses")
                    .with_spec_ref(spec.clone()),
            ))
        }
    }
}

/// A note recording which schema a report was produced against.
pub(crate) fn validated_against(
    document: &conform_core::DocumentId,
    spec: &SpecRef,
    provenance: &str,
) -> Diagnostic {
    Diagnostic::new(
        Severity::Info,
        codes::VALIDATED_AGAINST,
        Location::document(document.clone()),
        format!("validated against {spec}; {provenance}"),
    )
    .with_spec_ref(spec.clone())
}

/// Saturating `usize` → `u32`, matching `conform-core`'s choice of width for a
/// line number.
fn truncate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::compile;

    /// A `$ref` to an `https://` URL must not become a network fetch.
    ///
    /// This is the assertion behind `default-features = false` in this crate's
    /// manifest, and it is worth a test rather than a comment because the
    /// property is invisible at the call site: nothing in [`compile`] mentions
    /// the network, so nothing in [`compile`] would look wrong if a future
    /// version of `jsonschema` started resolving remote references by default.
    ///
    /// Why it matters here specifically. This crate validates documents against
    /// schema bytes whose SHA-256 is pinned in `specs.toml` and checked before
    /// use. A schema that could pull part of itself over the network would make
    /// that pin describe only the part that happened to be local, and — because
    /// the URL comes from the schema rather than from us — would let the
    /// document under test steer validation at an address of its choosing. The
    /// digest would still verify. It would just no longer mean anything.
    ///
    /// The host is `example.com`, which is reachable. So `Err` is evidence: had
    /// a retriever been compiled in, this would have resolved rather than
    /// refused.
    #[test]
    fn a_remote_ref_is_refused_rather_than_fetched() {
        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "properties": {
                "anything": { "$ref": "https://example.com/never-fetched.json" }
            }
        });

        // `let ... else` rather than `expect_err`: the `Ok` type here is
        // `jsonschema::Validator`, which is not `Debug`, so the combinators
        // that would unwrap this cannot be named.
        let Err(error) = compile(&schema) else {
            panic!("a schema with an unresolvable remote $ref must not compile");
        };

        let message = error.to_string();
        assert!(
            message.contains("https://example.com/never-fetched.json"),
            "the refusal should name the reference it would not follow, so a reader can see \
             which one: {message}"
        );
        assert!(
            message.contains("resolve-http"),
            "the refusal should say retrieval is switched off rather than merely that the \
             reference is absent, so this test fails loudly if the reason ever changes from \
             \"no retriever\" to \"fetch failed\": {message}"
        );
    }
}
