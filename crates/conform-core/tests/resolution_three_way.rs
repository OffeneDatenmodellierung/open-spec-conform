//! FR-005, and the reason it is a requirement rather than a preference.
//!
//! A cross-reference check has three outcomes, not two: the target resolved,
//! the target is genuinely absent from the inspected document set, or nobody
//! looked. The middle one is a finding. The last one is an absence of
//! evidence, and reporting it as a finding is how a gate learns to cry wolf —
//! after which it is switched off, and protects nothing.
//!
//! These assertions exist so that the distinction cannot be quietly removed
//! later by someone "simplifying" `Resolution` into an `Option`.

use std::collections::HashSet;

use conform_core::{NotInspectedReason, Resolution};

/// Every reason currently defined. `NotInspectedReason` is `#[non_exhaustive]`,
/// so this list is maintained by hand on purpose: adding a reason should be a
/// deliberate act that also decides what this test says about it.
const REASONS: &[NotInspectedReason] = &[
    NotInspectedReason::OutsideDocumentSet,
    NotInspectedReason::NotLoaded,
    NotInspectedReason::Excluded,
    NotInspectedReason::InspectionFailed,
    NotInspectedReason::Unsupported,
];

/// The only place in this test allowed to turn a `Resolution` into a summary.
/// Written as an exhaustive match so that adding a fourth outcome to
/// `Resolution` fails to compile here rather than silently falling into an
/// existing bucket.
fn outcome_tag<T>(resolution: &Resolution<T>) -> &'static str {
    match resolution {
        Resolution::Resolved(_) => "resolved",
        Resolution::DoesNotExist => "does-not-exist",
        Resolution::NotInspected { .. } => "not-inspected",
    }
}

#[test]
fn does_not_exist_and_not_inspected_are_not_equal() {
    let absent: Resolution<&str> = Resolution::DoesNotExist;

    for &reason in REASONS {
        let uninspected: Resolution<&str> = Resolution::not_inspected(reason);
        assert_ne!(
            absent, uninspected,
            "`DoesNotExist` compared equal to `NotInspected {{ reason: {reason} }}`: \
             \"the target is absent\" and \"nobody looked\" are different facts"
        );
        assert_ne!(outcome_tag(&absent), outcome_tag(&uninspected));
    }
}

#[test]
fn the_two_non_resolving_outcomes_answer_every_predicate_differently() {
    let absent: Resolution<&str> = Resolution::DoesNotExist;
    let uninspected: Resolution<&str> =
        Resolution::not_inspected(NotInspectedReason::OutsideDocumentSet);

    assert!(absent.does_not_exist());
    assert!(!absent.is_not_inspected());
    assert!(!absent.is_resolved());
    assert_eq!(absent.not_inspected_reason(), None);

    assert!(uninspected.is_not_inspected());
    assert!(!uninspected.does_not_exist());
    assert!(!uninspected.is_resolved());
    assert_eq!(
        uninspected.not_inspected_reason(),
        Some(NotInspectedReason::OutsideDocumentSet)
    );
}

/// `resolved()` narrows the *payload*, and both non-resolving outcomes have no
/// payload. That shared `None` is exactly the collapse this type prevents, so
/// it must not be reachable as a way of telling the two outcomes apart.
#[test]
fn an_empty_payload_does_not_merge_the_two_outcomes() {
    let absent: Resolution<&str> = Resolution::DoesNotExist;
    let uninspected: Resolution<&str> = Resolution::not_inspected(NotInspectedReason::NotLoaded);

    assert_eq!(absent.resolved(), None);
    assert_eq!(uninspected.resolved(), None);

    // Same payload answer, still different outcomes.
    assert_ne!(absent, uninspected);
    assert_ne!(outcome_tag(&absent), outcome_tag(&uninspected));
}

#[test]
fn every_reason_stays_distinct_from_every_other_and_from_absence() {
    let mut seen: HashSet<Resolution<&str>> = HashSet::new();
    assert!(seen.insert(Resolution::Resolved("target")));
    assert!(seen.insert(Resolution::DoesNotExist));

    for &reason in REASONS {
        assert!(
            seen.insert(Resolution::not_inspected(reason)),
            "`NotInspected {{ reason: {reason} }}` hashed or compared equal to an outcome already \
             recorded; reasons must not merge into each other or into `DoesNotExist`"
        );
    }

    assert_eq!(seen.len(), REASONS.len() + 2);

    let names: HashSet<&str> = REASONS.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        names.len(),
        REASONS.len(),
        "two reasons share a machine-readable name"
    );
}

#[test]
fn transformations_preserve_the_distinction() {
    let absent: Resolution<u32> = Resolution::DoesNotExist;
    let uninspected: Resolution<u32> =
        Resolution::not_inspected(NotInspectedReason::InspectionFailed);

    // `map` touches the payload only.
    assert_eq!(absent.map(|n| n + 1), Resolution::DoesNotExist);
    assert_eq!(
        uninspected.map(|n| n + 1),
        Resolution::not_inspected(NotInspectedReason::InspectionFailed)
    );
    assert_eq!(
        Resolution::Resolved(1).map(|n| n + 1),
        Resolution::Resolved(2)
    );

    // ...and so does borrowing.
    assert_eq!(absent.by_ref(), Resolution::DoesNotExist);
    assert_eq!(
        uninspected.by_ref(),
        Resolution::not_inspected(NotInspectedReason::InspectionFailed)
    );
    assert_eq!(Resolution::Resolved(1).by_ref(), Resolution::Resolved(&1));
}

/// Serialization must carry the distinction too: a consumer reading a
/// serialized report has to be able to tell "absent" from "not inspected",
/// otherwise the collapse simply happens one layer further out.
///
/// Asserted at the type level rather than against a concrete encoding, because
/// pulling a serialization format in as a dev-dependency would put a crate
/// other than `serde` into the manifest of the one crate in this family whose
/// dependency tree is everybody else's (spec NFR-001). The three variants'
/// on-the-wire names are covered by `every_reason_stays_distinct_from_every_\
/// other_and_from_absence` via `NotInspectedReason::as_str`.
#[cfg(feature = "serde")]
#[test]
fn the_serde_feature_covers_the_three_way_type() {
    fn assert_round_trippable<T: serde::Serialize + serde::de::DeserializeOwned>() {}

    assert_round_trippable::<Resolution<String>>();
    assert_round_trippable::<NotInspectedReason>();
}
