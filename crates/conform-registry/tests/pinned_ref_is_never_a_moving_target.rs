//! The rule this crate is named for, and the reason it is a test rather than a
//! convention.
//!
//! `data-modelling-sdk/schemas/odps-json-schema-latest.json` is a vendored copy
//! of a document that upstream keeps changing, pinned to the word "latest". It
//! has no version, no source URL, no fetch date and no `$id`, so the question
//! "is this current?" has no answer short of hand-diffing against Bitol. The
//! file is not an oversight by somebody careless; it is what happens by default
//! when nothing enforces the alternative.
//!
//! So this enforces the alternative. The check runs over the real `specs.toml`
//! on every `cargo test`, and the negative controls below prove it actually
//! bites — a guard that passes because it never fires is the same as no guard,
//! and rather more expensive, because it is believed.

mod support;

use conform_core::{GatePolicy, Severity};
use conform_registry::{MOVING_REFS, Registry, codes, is_moving_ref};

/// The one question, asked of a registry: which entries are pinned to a moving
/// target? Both the real test and the negative controls go through this, so
/// they cannot drift apart into "the control tests a different code path".
fn moving_pins(registry: &Registry) -> Vec<String> {
    registry
        .validate()
        .into_iter()
        .filter(|d| d.code.as_str() == codes::MOVING_REF)
        .map(|d| format!("{} — {}", d.location, d.message))
        .collect()
}

#[test]
fn the_real_registry_pins_nothing_to_a_moving_target() {
    let registry = support::real_registry();
    assert!(
        !registry.entries().is_empty(),
        "an empty registry would pass every check here while proving nothing"
    );

    let offenders = moving_pins(&registry);
    assert!(
        offenders.is_empty(),
        "specs.toml pins {} entr{} to a moving target:\n  {}",
        offenders.len(),
        if offenders.len() == 1 { "y" } else { "ies" },
        offenders.join("\n  ")
    );
}

/// The negative control for the test above: the same function, over a registry
/// that says `latest`, must find it.
#[test]
fn negative_control_a_registry_that_says_latest_is_caught() {
    for spelling in ["latest", "LATEST", "Latest", "  latest  "] {
        let registry = registry_pinned_to(spelling);
        let offenders = moving_pins(&registry);

        assert_eq!(
            offenders.len(),
            1,
            "`pinned_ref = \"{spelling}\"` was not reported as a moving target; \
             the real-registry test above is therefore worthless"
        );
        assert!(
            registry.validate().should_gate(GatePolicy::default()),
            "`pinned_ref = \"{spelling}\"` was reported but does not gate"
        );
    }
}

/// The same control applied to the real file rather than to a hand-written
/// fixture: take `specs.toml` as it ships, change one pin to `latest`, and the
/// check must fail. This is what proves the rule is wired to the document the
/// first test reads, and not to a convenient copy of it.
#[test]
fn negative_control_the_real_registry_with_one_pin_swapped_for_latest_is_caught() {
    let text = support::real_registry_text();
    let original = "pinned_ref    = \"v1.0.0\"";
    assert!(
        text.contains(original),
        "the ODPS pin is no longer spelled `{original}`; update this control so it \
         keeps testing the real file"
    );

    let sabotaged = text.replacen(original, "pinned_ref    = \"latest\"", 1);
    let registry = Registry::load_str(&sabotaged, "<specs.toml with the ODPS pin sabotaged>")
        .expect("the sabotaged registry should still parse — it is the rules that must reject it");

    let offenders = moving_pins(&registry);
    assert_eq!(
        offenders.len(),
        1,
        "specs.toml with the ODPS pin set back to \"latest\" was accepted: the exact \
         defect this crate exists to prevent would be reintroducible"
    );
    assert!(offenders[0].contains("/spec/1/pinned_ref"));
}

#[test]
fn every_moving_ref_is_rejected_whatever_its_case() {
    for moving in MOVING_REFS {
        for spelling in [moving.to_string(), moving.to_uppercase()] {
            assert!(is_moving_ref(&spelling), "`{spelling}` should be moving");

            let registry = registry_pinned_to(&spelling);
            let report = registry.validate();
            assert_eq!(
                report.count(Severity::Error),
                1,
                "`pinned_ref = \"{spelling}\"` produced {report:?}"
            );
        }
    }
}

/// The positive control: real pins must not trip the rule, or the rule would be
/// noise and get switched off.
#[test]
fn immutable_pins_are_accepted() {
    for pin in [
        "v1.0.0",
        "3.1.0",
        "ad30107c31c06aec8a7d5636e0d1058118604e6f",
        "release-2026-01",
        "v2.0.0-rc.1",
    ] {
        let registry = registry_pinned_to(pin);
        assert!(
            moving_pins(&registry).is_empty(),
            "`pinned_ref = \"{pin}\"` is immutable and must be accepted"
        );
    }
}

/// An entry that omits `pinned_ref` is a *warning*, never an error, and never
/// a moving-target finding. "We have not established which revision this is"
/// and "we wrote down a pointer that cannot mean one revision" are different
/// statements, and collapsing them would make the honest one unsayable.
#[test]
fn an_absent_pin_is_reported_but_is_not_a_moving_target() {
    let registry = Registry::load_str(
        r#"
        schema_version = 1

        [[spec]]
        id            = "unpinned"
        name          = "A specification whose upstream revision is not known"
        vendored_path = "schemas/unpinned.json"
        sha256        = "0000000000000000000000000000000000000000000000000000000000000000"
        fetched_at    = "2026-09-20"
        notes         = "Upstream publishes no tags and the fetch predates this registry."
        "#,
        "<unpinned>",
    )
    .expect("the registry parses");

    let report = registry.validate();
    assert!(moving_pins(&registry).is_empty());
    assert_eq!(report.count(Severity::Error), 0, "{report:?}");
    assert!(
        report
            .iter()
            .any(|d| d.code.as_str() == codes::UNPINNED && d.severity == Severity::Warning),
        "an absent pin must still be reported: {report:?}"
    );
}

/// A fixture registry holding exactly one entry with the given pin, and
/// nothing else wrong with it.
fn registry_pinned_to(pin: &str) -> Registry {
    let text = format!(
        r#"
        schema_version = 1

        [[spec]]
        id            = "fixture"
        name          = "Fixture Specification"
        homepage      = "https://example.invalid/fixture"
        repository    = "https://example.invalid/fixture.git"
        steward       = "Nobody"
        licence       = "Apache-2.0"
        pinned_ref    = "{pin}"
        vendored_path = "schemas/fixture.json"
        sha256        = "0000000000000000000000000000000000000000000000000000000000000000"
        fetched_at    = "2026-09-20"
        "#
    );

    Registry::load_str(&text, "<fixture>").expect("the fixture registry parses")
}
