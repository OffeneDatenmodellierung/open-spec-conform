//! `conform-core` asks for one thing from an adapter's codes: that they are
//! stable. A test cannot check stability across releases, but it can check the
//! two properties that make stability *possible* — that no code is used for
//! two different rules, and that a report holding findings from four adapters
//! at once can tell whose is whose.
//!
//! The list is read out of `src/codes.rs` rather than restated here. A list
//! restated in a test is a second place to forget, and the failure mode — a
//! new code added to the crate and not to the test — is the exact one that
//! makes the test stop meaning anything.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// Every `pub const NAME: &str = "CODE";` in `src/codes.rs`, as
/// `(constant name, code)`.
fn declared_codes() -> Vec<(String, String)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/codes.rs");
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("pub const ")?;
            let (name, rest) = rest.split_once(": &str = ")?;
            let code = rest.trim().trim_end_matches(';').trim_matches('"');
            Some((name.to_owned(), code.to_owned()))
        })
        .collect()
}

/// A scanner that finds nothing passes every assertion below vacuously.
#[test]
fn the_scan_actually_reaches_the_codes() {
    let codes = declared_codes();
    assert!(
        codes.len() >= 40,
        "found only {} codes in src/codes.rs, which is fewer than this crate declares",
        codes.len()
    );
    assert!(
        codes
            .iter()
            .any(|(name, code)| name == "TYPE_MISSING" && code == "OKF002"),
        "the scan did not find a code it is known to declare"
    );
}

/// No code names two rules. Reusing one is the one edit that silently breaks
/// every suppression list downstream, because nothing about it looks wrong.
#[test]
fn no_code_is_used_twice() {
    let mut by_code: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, code) in declared_codes() {
        by_code.entry(code).or_default().push(name);
    }

    let collisions: Vec<String> = by_code
        .iter()
        .filter(|(_, names)| names.len() > 1)
        .map(|(code, names)| format!("{code} is used by {}", names.join(", ")))
        .collect();

    assert!(
        collisions.is_empty(),
        "two rules share a code. A code names a kind of finding and nothing else; retire one \
         rather than reuse it.\n  {}",
        collisions.join("\n  ")
    );
}

/// Every code carries this crate's prefix.
///
/// A report can hold findings from several adapters at once, and an
/// unprefixed code in that report names nothing. This is also what keeps the
/// hygiene rules from colliding with another adapter that decides to number
/// its own rules `L1`.
#[test]
fn every_code_is_prefixed_and_shaped_like_a_code() {
    for (name, code) in declared_codes() {
        assert!(
            code.starts_with("OKF"),
            "{name} = {code:?} does not carry this crate's prefix"
        );
        assert!(
            code.len() >= 6
                && code
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
            "{name} = {code:?} is not an upper-case alphanumeric code"
        );
    }
}

/// The hygiene table names real codes, covers every hygiene rule exactly
/// once, and maps each to a distinct upstream identifier.
///
/// This table is the migration's only record of something that would
/// otherwise be lost: the implementation this crate replaces put upstream's
/// `L1`..`L12` in the finding's code field, and a consumer who learned those
/// identifiers needs a way to follow them.
#[test]
fn the_hygiene_table_accounts_for_every_hygiene_rule() {
    let declared: BTreeMap<String, String> = declared_codes().into_iter().collect();

    let hygiene_codes: Vec<&str> = declared
        .values()
        .filter(|code| code.starts_with("OKFL") || code.starts_with("OKFR"))
        .map(String::as_str)
        .collect();

    let tabled: Vec<&str> = conform_okf::codes::HYGIENE_RULES
        .iter()
        .map(|(code, _)| *code)
        .collect();

    let mut hygiene_sorted = hygiene_codes.clone();
    hygiene_sorted.sort_unstable();
    let mut tabled_sorted = tabled.clone();
    tabled_sorted.sort_unstable();
    assert_eq!(
        hygiene_sorted, tabled_sorted,
        "HYGIENE_RULES and the hygiene codes in src/codes.rs disagree"
    );

    let mut rules: Vec<&str> = conform_okf::codes::HYGIENE_RULES
        .iter()
        .map(|(_, rule)| *rule)
        .collect();
    let before = rules.len();
    rules.sort_unstable();
    rules.dedup();
    assert_eq!(
        rules.len(),
        before,
        "two hygiene codes claim the same upstream rule identifier"
    );

    // And a conformance code is not in the table, because it never had a rule
    // identifier to carry forward. That gap is recorded, not papered over.
    assert_eq!(
        conform_okf::codes::upstream_rule(conform_okf::codes::TYPE_MISSING),
        None
    );
    assert_eq!(
        conform_okf::codes::upstream_rule(conform_okf::codes::DRAFT),
        Some("L12")
    );
}
