//! What the registry does not know, written down, so that filling a gap in is
//! a deliberate act rather than a quiet one.
//!
//! The rule the registry enforces is that an unknown must be *recorded* as
//! unknown and explained in `notes` — never guessed at. This test is the other
//! half of that rule: it pins the exact set of gaps that exist today. Learn a
//! licence and this test fails until somebody updates the ledger; invent one
//! and it fails too. Either way the change is visible in a diff instead of
//! appearing as one more confident-looking string in a TOML file.

mod support;

use conform_core::{GatePolicy, Severity};
use conform_registry::{ProvenanceRules, codes};

/// Every provenance field this registry does not record, as `id/field`.
///
/// Each of these is an honest gap with a reason in the entry's `notes`. None of
/// them is a value that could have been looked up in this estate and was not.
const KNOWN_GAPS: &[&str] = &[
    // No licence is recorded for any of the three third-party schemas anywhere
    // in `data-modelling-sdk` — not in the schema, not in its README, not in
    // that repository's LICENSE, which covers its own code. Establishing them
    // means reading the upstream repositories, which needs network access this
    // work did not have.
    "odcs/licence",
    "odps/licence",
    "odcl/licence",
    // The schemas README records only a repository blob URL for this one. A
    // documentation site may well exist; guessing its address is the failure
    // this crate exists to prevent.
    "odcl/homepage",
    // The only evidence available is the GitHub organisation in the repository
    // URL, which names an account rather than a maintaining body.
    "odcl/steward",
    // First-party and unpublished: the `$id` in the document is an
    // `example.org` placeholder, not a resolvable address.
    "cads/homepage",
];

#[test]
fn the_registry_records_exactly_the_gaps_it_claims_to() {
    let registry = support::real_registry();

    let mut actual: Vec<String> = registry
        .entries()
        .iter()
        .flat_map(|entry| {
            entry
                .provenance_gaps()
                .into_iter()
                .map(move |gap| format!("{}/{gap}", entry.id))
        })
        .collect();
    actual.sort();

    let mut expected: Vec<String> = KNOWN_GAPS.iter().map(ToString::to_string).collect();
    expected.sort();

    assert_eq!(
        actual, expected,
        "the set of unknowns in specs.toml changed. If a gap was closed, delete it from \
         KNOWN_GAPS and say in the entry's notes what evidence closed it. If a gap \
         appeared, add it here with the same reasoning. What must not happen is a gap \
         being filled with a plausible value nobody checked."
    );
}

#[test]
fn the_registry_passes_its_own_rules() {
    let registry = support::real_registry();
    let report = registry.validate();

    assert_eq!(
        report.count(Severity::Error),
        0,
        "specs.toml breaks its own rules:\n  {}",
        report
            .at_or_above(Severity::Error)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    assert!(!report.should_gate(GatePolicy::default()));
}

/// Every gap is warned about, and every entry that has one explains it. The
/// warnings are expected output, not a failure — the point is that they are
/// *said out loud* on every run rather than living in somebody's memory.
#[test]
fn every_gap_is_both_reported_and_explained() {
    let registry = support::real_registry();
    let report = registry.validate();

    let reported = report
        .iter()
        .filter(|d| {
            matches!(
                d.code.as_str(),
                c if c == codes::PROVENANCE_GAP || c == codes::UNPINNED
            )
        })
        .count();
    assert_eq!(
        reported,
        KNOWN_GAPS.len(),
        "each known gap should produce exactly one warning: {report:?}"
    );

    assert_eq!(
        report.count(Severity::Error),
        0,
        "an unexplained gap is an error (REG004), and none should be unexplained"
    );

    for entry in registry.entries() {
        if !entry.provenance_gaps().is_empty() {
            let notes = entry.notes.as_deref().unwrap_or_default();
            assert!(
                !notes.trim().is_empty(),
                "`{}` has gaps and no notes explaining them",
                entry.id
            );
        }
    }
}

/// The registry's rules run through `conform-core`'s `Validator` like any
/// other validator in this family: same trait, same report, same gate.
#[test]
fn the_rules_are_a_validator_like_any_other() {
    use conform_core::Validator as _;

    let registry = support::real_registry();
    let verdict = ProvenanceRules.check(&registry, GatePolicy::default());

    assert!(!verdict.is_gated());
    assert_eq!(verdict.exit_code(), 0);
    assert!(
        ProvenanceRules
            .check(&registry, GatePolicy::warnings_as_errors())
            .is_gated(),
        "the gaps are real warnings, so a stricter policy must gate on them — otherwise \
         the warnings are decorative"
    );
}

/// Every entry pins to an immutable revision, and says which.
#[test]
fn every_entry_is_pinned() {
    for entry in support::real_registry().entries() {
        assert!(
            entry.is_pinned(),
            "`{}` records no immutable upstream revision",
            entry.id
        );
    }
}
