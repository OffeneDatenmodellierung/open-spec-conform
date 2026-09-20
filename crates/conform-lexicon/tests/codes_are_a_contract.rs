//! The diagnostic codes are a public API, and this is what makes that true.
//!
//! `conform-core` asks adapters to treat their codes as stable: downstream
//! tooling, CI annotations and suppression lists match on these strings, so
//! the message attached to a code may be reworded freely and the code itself
//! may not. "Treat them as stable" is a request until something checks it.
//!
//! This file spells every code out literally. A rename — even a tidy-up, even
//! one that keeps the constant's name — fails here, which turns "I changed a
//! code" from something that happens in passing into something somebody
//! decided to do and had to change a test to say so.

mod support;

use std::collections::BTreeSet;

use conform_core::{Severity, Validator};
use conform_lexicon::codes;

/// Every code this crate may raise, and its exact string.
///
/// Written out rather than derived from the constants, because a test that
/// asserted `codes::REQUIRED_MISSING == codes::REQUIRED_MISSING` would be
/// checking nothing at all.
const PUBLISHED: &[(&str, &str)] = &[
    ("UNPARSEABLE", "ODCL001"),
    ("EMPTY", "ODCL002"),
    ("SCHEMA_VIOLATION", "ODCL100"),
    ("REQUIRED_MISSING", "ODCL101"),
    ("UNKNOWN_PROPERTY", "ODCL102"),
    ("VALUE_NOT_PERMITTED", "ODCL103"),
    ("WRONG_TYPE", "ODCL104"),
    ("MALFORMED_VALUE", "ODCL105"),
    ("BAD_CARDINALITY", "ODCL106"),
    ("OUT_OF_RANGE", "ODCL107"),
    ("NO_MATCHING_ALTERNATIVE", "ODCL108"),
    ("SPEC_VERSION_BEHIND", "ODCL200"),
    ("NO_MODELS", "ODCL201"),
    ("NO_SERVERS", "ODCL202"),
    ("NO_OWNER", "ODCL203"),
    ("NO_DESCRIPTION", "ODCL204"),
    ("UNCONVENTIONAL_STATUS", "ODCL205"),
    ("UNKNOWN_ROOT_KEY", "ODCL206"),
    ("UNTYPED_FIELD", "ODCL207"),
    ("DEPRECATED_KEYWORD", "ODCL208"),
    ("REFERENCE_DOES_NOT_EXIST", "ODCL300"),
    ("DEFINITION_DOES_NOT_EXIST", "ODCL301"),
    ("REFERENCE_NOT_INSPECTED", "ODCL302"),
    ("REFERENCE_RESOLVED", "ODCL303"),
    ("SERVER_TYPE_NOT_INSPECTED", "ODCL304"),
    ("NOT_IN_REGISTRY", "ODCL900"),
    ("SCHEMA_UNREADABLE", "ODCL901"),
    ("SCHEMA_UNUSABLE", "ODCL902"),
    ("SCHEMA_PROVENANCE_FAILED", "ODCL903"),
    ("VALIDATED_AGAINST", "ODCL904"),
];

/// The codes in the `9xx` and `3xx` bands that are information rather than a
/// complaint, spelled out so the exception cannot spread quietly.
const INFORMATIONAL: &[&str] = &[
    codes::REFERENCE_NOT_INSPECTED,
    codes::REFERENCE_RESOLVED,
    codes::SERVER_TYPE_NOT_INSPECTED,
    codes::VALIDATED_AGAINST,
];

#[test]
fn the_published_codes_have_not_moved() {
    let actual: &[(&str, &str)] = &[
        ("UNPARSEABLE", codes::UNPARSEABLE),
        ("EMPTY", codes::EMPTY),
        ("SCHEMA_VIOLATION", codes::SCHEMA_VIOLATION),
        ("REQUIRED_MISSING", codes::REQUIRED_MISSING),
        ("UNKNOWN_PROPERTY", codes::UNKNOWN_PROPERTY),
        ("VALUE_NOT_PERMITTED", codes::VALUE_NOT_PERMITTED),
        ("WRONG_TYPE", codes::WRONG_TYPE),
        ("MALFORMED_VALUE", codes::MALFORMED_VALUE),
        ("BAD_CARDINALITY", codes::BAD_CARDINALITY),
        ("OUT_OF_RANGE", codes::OUT_OF_RANGE),
        ("NO_MATCHING_ALTERNATIVE", codes::NO_MATCHING_ALTERNATIVE),
        ("SPEC_VERSION_BEHIND", codes::SPEC_VERSION_BEHIND),
        ("NO_MODELS", codes::NO_MODELS),
        ("NO_SERVERS", codes::NO_SERVERS),
        ("NO_OWNER", codes::NO_OWNER),
        ("NO_DESCRIPTION", codes::NO_DESCRIPTION),
        ("UNCONVENTIONAL_STATUS", codes::UNCONVENTIONAL_STATUS),
        ("UNKNOWN_ROOT_KEY", codes::UNKNOWN_ROOT_KEY),
        ("UNTYPED_FIELD", codes::UNTYPED_FIELD),
        ("DEPRECATED_KEYWORD", codes::DEPRECATED_KEYWORD),
        ("REFERENCE_DOES_NOT_EXIST", codes::REFERENCE_DOES_NOT_EXIST),
        (
            "DEFINITION_DOES_NOT_EXIST",
            codes::DEFINITION_DOES_NOT_EXIST,
        ),
        ("REFERENCE_NOT_INSPECTED", codes::REFERENCE_NOT_INSPECTED),
        ("REFERENCE_RESOLVED", codes::REFERENCE_RESOLVED),
        (
            "SERVER_TYPE_NOT_INSPECTED",
            codes::SERVER_TYPE_NOT_INSPECTED,
        ),
        ("NOT_IN_REGISTRY", codes::NOT_IN_REGISTRY),
        ("SCHEMA_UNREADABLE", codes::SCHEMA_UNREADABLE),
        ("SCHEMA_UNUSABLE", codes::SCHEMA_UNUSABLE),
        ("SCHEMA_PROVENANCE_FAILED", codes::SCHEMA_PROVENANCE_FAILED),
        ("VALIDATED_AGAINST", codes::VALIDATED_AGAINST),
    ];

    assert_eq!(
        actual, PUBLISHED,
        "a diagnostic code changed. These strings are matched on by tooling outside this \
         repository, so changing one is a breaking change to this crate's public API — do it \
         deliberately, with a major version, and update this list to say so."
    );
}

#[test]
fn every_code_is_distinct() {
    let unique: BTreeSet<&str> = PUBLISHED.iter().map(|(_, code)| *code).collect();
    assert_eq!(
        unique.len(),
        PUBLISHED.len(),
        "two rules share a code, so a consumer cannot tell their findings apart"
    );
}

#[test]
fn every_code_is_namespaced_to_this_standard() {
    for (name, code) in PUBLISHED {
        assert!(
            code.starts_with("ODCL"),
            "{name} is `{code}`, which is not namespaced to this standard; a bare number would \
             collide with the sibling adapters the moment two reports were merged"
        );
    }
    // And the namespace is the registry's id for this standard, upper-cased.
    // A code prefix that drifts from the id it accompanies is a code prefix
    // nobody can trace back to a provenance record.
    assert_eq!(
        conform_lexicon::SPEC_ID.to_uppercase(),
        "ODCL",
        "the code prefix and the registry id have come apart"
    );
}

/// The bands are a promise too: the crate documentation tells a reader they
/// can infer severity from the number, so nothing may quietly cross a band.
#[test]
fn severity_follows_the_documented_bands() {
    let validator = support::validator();

    for path in support::fixtures() {
        let name = path
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let report = validator.validate(&support::fixture(&name));

        for diagnostic in &report {
            let code = diagnostic.code.as_str();
            let expected = if INFORMATIONAL.contains(&code) {
                Severity::Info
            } else {
                match &code[4..5] {
                    "0" | "1" | "9" => Severity::Error,
                    "2" | "3" => Severity::Warning,
                    other => panic!("{code} is in band {other}, which is not documented"),
                }
            };
            assert_eq!(
                diagnostic.severity, expected,
                "{name}: {code} was raised as {} and its band says {expected}",
                diagnostic.severity
            );
        }
    }
}

/// Every code this crate publishes should be reachable, or it is documentation
/// of a rule that does not exist.
///
/// The exceptions are named and argued rather than allowed silently: each is a
/// code whose trigger is a broken *environment* rather than a document, and
/// `tests/provenance_is_enforced.rs` covers all but one of those with a
/// scratch registry.
#[test]
fn the_corpus_reaches_the_codes_that_describe_documents() {
    /// Codes no fixture can reach, because they are not about a document.
    const NOT_ABOUT_A_DOCUMENT: &[&str] = &[
        // A schema keyword class the vendored schema does not use anywhere.
        // Kept as the catch-all so a future keyword is reported rather than
        // dropped — see its documentation.
        codes::SCHEMA_VIOLATION,
        // The ODCL root does not close `additionalProperties`, so this crate
        // cannot raise an unknown-property ERROR at the root; `UNKNOWN_ROOT_KEY`
        // is the warning that covers the gap. Nested objects that do close it
        // exist, but no fixture needs one to make a point twice.
        codes::UNKNOWN_PROPERTY,
        // The vendored schema uses no `minItems`/`uniqueItems`-class keyword
        // on anything a fixture reaches, and none of the range keywords.
        codes::BAD_CARDINALITY,
        codes::OUT_OF_RANGE,
        codes::NO_MATCHING_ALTERNATIVE,
        // Setup failures. Covered by tests/provenance_is_enforced.rs.
        codes::NOT_IN_REGISTRY,
        codes::SCHEMA_UNREADABLE,
        codes::SCHEMA_UNUSABLE,
        codes::SCHEMA_PROVENANCE_FAILED,
    ];

    let validator = support::validator();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for path in support::fixtures() {
        let name = path
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        seen.extend(support::codes_in(
            &validator.validate(&support::fixture(&name)),
        ));
    }

    let unreached: Vec<&str> = PUBLISHED
        .iter()
        .map(|(_, code)| *code)
        .filter(|code| !seen.contains(*code) && !NOT_ABOUT_A_DOCUMENT.contains(code))
        .collect();
    assert!(
        unreached.is_empty(),
        "these codes are published but no fixture reaches them, so nothing shows they are \
         raised correctly — add a fixture, or argue the code into NOT_ABOUT_A_DOCUMENT with a \
         reason: {unreached:?}"
    );

    // The control, so the exemption list cannot quietly grow to cover a rule
    // that stopped working.
    for code in NOT_ABOUT_A_DOCUMENT {
        assert!(
            !seen.contains(*code),
            "{code} is listed as unreachable from a document and a fixture reached it — good \
             news, and it means the list is out of date"
        );
    }
}
