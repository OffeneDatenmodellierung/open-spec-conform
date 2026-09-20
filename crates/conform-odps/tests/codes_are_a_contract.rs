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

use conform_core::Validator;
use conform_odps::codes;

/// Every code this crate may raise, and its exact string.
///
/// Written out rather than derived from the constants, because a test that
/// asserted `codes::REQUIRED_MISSING == codes::REQUIRED_MISSING` would be
/// checking nothing at all.
const PUBLISHED: &[(&str, &str)] = &[
    ("UNPARSEABLE", "ODPS001"),
    ("EMPTY", "ODPS002"),
    ("SCHEMA_VIOLATION", "ODPS100"),
    ("REQUIRED_MISSING", "ODPS101"),
    ("UNKNOWN_PROPERTY", "ODPS102"),
    ("VALUE_NOT_PERMITTED", "ODPS103"),
    ("WRONG_TYPE", "ODPS104"),
    ("MALFORMED_VALUE", "ODPS105"),
    ("BAD_CARDINALITY", "ODPS106"),
    ("OUT_OF_RANGE", "ODPS107"),
    ("NO_MATCHING_ALTERNATIVE", "ODPS108"),
    ("API_VERSION_BEHIND", "ODPS200"),
    ("NO_OUTPUT_PORTS", "ODPS201"),
    ("NO_INPUT_PORTS", "ODPS202"),
    ("NO_TEAM", "ODPS203"),
    ("NO_DESCRIPTION", "ODPS204"),
    ("UNCONVENTIONAL_STATUS", "ODPS205"),
    ("NO_VERSION", "ODPS206"),
    ("NOT_IN_REGISTRY", "ODPS900"),
    ("SCHEMA_UNREADABLE", "ODPS901"),
    ("SCHEMA_UNUSABLE", "ODPS902"),
    ("SCHEMA_PROVENANCE_FAILED", "ODPS903"),
    ("VALIDATED_AGAINST", "ODPS904"),
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
        ("API_VERSION_BEHIND", codes::API_VERSION_BEHIND),
        ("NO_OUTPUT_PORTS", codes::NO_OUTPUT_PORTS),
        ("NO_INPUT_PORTS", codes::NO_INPUT_PORTS),
        ("NO_TEAM", codes::NO_TEAM),
        ("NO_DESCRIPTION", codes::NO_DESCRIPTION),
        ("UNCONVENTIONAL_STATUS", codes::UNCONVENTIONAL_STATUS),
        ("NO_VERSION", codes::NO_VERSION),
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
            code.starts_with("ODPS"),
            "{name} is `{code}`, which is not namespaced to this standard; a bare number would \
             collide with the sibling adapters the moment two reports were merged"
        );
    }
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
            let expected = match &code[4..5] {
                // The provenance note is the one finding in the 9xx band that
                // is not a refusal: it says the schema WAS checked. Everything
                // else about the validator's own setup is a refusal to run.
                "9" if code == codes::VALIDATED_AGAINST => conform_core::Severity::Info,
                "0" | "1" | "9" => conform_core::Severity::Error,
                "2" => conform_core::Severity::Warning,
                other => panic!("{code} is in band {other}, which is not documented"),
            };
            assert_eq!(
                diagnostic.severity, expected,
                "{name}: {code} was raised as {} and its band says {expected}",
                diagnostic.severity
            );
        }
    }
}
