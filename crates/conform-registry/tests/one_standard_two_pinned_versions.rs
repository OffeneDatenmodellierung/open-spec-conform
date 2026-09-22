//! One specification, catalogued twice, because the format gives a standard
//! exactly one row.
//!
//! # What this pins, and why it is a test rather than a comment
//!
//! `ossie` and `ossie-dev` are the same specification at two upstream
//! revisions: the only released version, and the in-development draft
//! implementers are actually building against. The entry format has one `id`
//! per entry and one `version` per entry, and `Registry::validate` refuses
//! two entries that share an `id` (`REG006`) — so carrying two pinned versions of one
//! standard means two identities, and the second identity is a version wearing
//! an identity field's clothes.
//!
//! That is a stopgap, and the risk with a stopgap is not that it is wrong on
//! the day it is written; it is that nobody afterwards can tell it apart from
//! a decision. So the shape it takes is asserted here: the two entries agree
//! on everything that names a *standard* and disagree on everything that names
//! a *revision*. If somebody later extends the format so that one entry can
//! carry many pinned versions — the model this should have had — these tests
//! are what they delete, and deleting them is how the next reader learns the
//! stopgap is over.
//!
//! # And the rule it must not weaken
//!
//! The duplicate-`id` rule is what makes the stopgap necessary, so the
//! temptation is to relax it. [`the_duplicate_id_rule_still_bites`] is the
//! negative control: it renames the second entry onto the first's `id` and
//! requires the registry to refuse. A registry that accepted two entries
//! called `ossie` would have no way to say which one a diagnostic's `SpecRef`
//! pointed at.

mod support;

use conform_core::Severity;
use conform_registry::{Registry, SpecEntry, codes};

/// The released entry and the in-development one, in that order.
fn both() -> (SpecEntry, SpecEntry) {
    let registry = support::real_registry();
    let released = registry
        .find("ossie")
        .expect("the registry records the released version")
        .clone();
    let development = registry
        .find("ossie-dev")
        .expect("the registry records the in-development version")
        .clone();
    (released, development)
}

#[test]
fn the_two_entries_agree_on_the_standard_and_differ_on_the_revision() {
    let (released, development) = both();

    // Everything that names the specification itself. A difference in any of
    // these would mean these are two standards rather than two revisions of
    // one, and the case for a single entry carrying both would evaporate.
    assert_eq!(released.name, development.name);
    assert_eq!(released.repository, development.repository);
    assert_eq!(released.homepage, development.homepage);
    assert_eq!(released.steward, development.steward);
    assert_eq!(released.licence, development.licence);

    // Everything that names one revision of it. These are the fields a
    // multi-version entry would hold a list of.
    assert_ne!(released.id, development.id);
    assert_ne!(released.version, development.version);
    assert_ne!(released.pinned_ref, development.pinned_ref);
    assert_ne!(released.vendored_path, development.vendored_path);
    assert_ne!(
        released.sha256, development.sha256,
        "two revisions of a document that hash alike are one revision recorded twice"
    );
}

#[test]
fn the_in_development_entry_is_polled_more_often_than_the_released_one() {
    // Not decoration. The released entry is pinned to a tag that upstream
    // cannot move; the other is pinned to a commit on a branch that moved
    // twice in the month it was vendored in, and a poll that runs monthly over
    // a file that changes fortnightly reports "no change" for a fortnight at a
    // time — which is the same false green the pin itself exists to prevent.
    let (released, development) = both();

    let cadence = |entry: &SpecEntry| {
        entry
            .poll
            .as_ref()
            .and_then(|poll| poll.cadence.clone())
            .unwrap_or_else(|| panic!("`{}` records no poll cadence", entry.id))
    };

    assert_eq!(cadence(&development), "weekly");
    assert_eq!(cadence(&released), "monthly");
}

#[test]
fn the_duplicate_id_rule_still_bites() {
    let text = support::real_registry_text();
    let sabotaged = text.replacen(
        r#"id            = "ossie-dev""#,
        r#"id            = "ossie""#,
        1,
    );
    assert_ne!(
        sabotaged, text,
        "the second entry is no longer spelled `ossie-dev`; update this control"
    );

    let registry = Registry::load_str(&sabotaged, "<specs.toml with a repeated id>")
        .expect("the sabotaged registry parses")
        .with_root(support::workspace_root());
    let report = registry.validate();

    assert_eq!(
        report.count(Severity::Error),
        1,
        "two entries sharing an id went unremarked: {report:?}"
    );
    assert!(
        report
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == codes::DUPLICATE_ID)
    );

    // And the real registry has no such error, so the assertion above is about
    // the sabotage rather than about a rule that fires on everything.
    assert_eq!(
        support::real_registry().validate().count(Severity::Error),
        0
    );
}
