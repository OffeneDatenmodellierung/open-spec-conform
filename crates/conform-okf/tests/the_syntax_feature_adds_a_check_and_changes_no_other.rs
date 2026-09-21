//! Turning `syntax` on adds a check. It does not edit either of the other two.
//!
//! # The defect this exists to prevent
//!
//! A feature that changes what an existing check reports makes that check a
//! function of the build configuration as well as of the bundle. Two consumers
//! on the same version of this crate would then get different findings over
//! the same bytes, with nothing in either report saying why — and the one who
//! got fewer would have no way to know they were reading a weaker answer.
//! `conform-core` states that rule for its `serde` feature; the reasoning has
//! nothing to do with serialization.
//!
//! The rules here could have been added to `OkfHygiene` behind a `cfg`. They
//! were not, and this file is the difference being enforced rather than
//! intended: `OkfConformance` and `OkfHygiene` say the same thing in every
//! feature configuration, and the `syntax` features add a third validator that
//! a caller has to name.
//!
//! # This file compiles in every configuration, deliberately
//!
//! Including the default one, where `OkfSyntax` does not exist. That is the
//! configuration where the guarantee is easiest to break silently, because
//! nothing in a default build would otherwise mention the syntax codes at all.
//!
//! # What this does not have to prove
//!
//! That the *messages* of the other two checks are unchanged.
//! `upstream_bundles_are_checked_the_same_way` pins both reports in full, over
//! both vendored bundles, and it runs in whatever configuration the suite is
//! run in — so `cargo test --workspace --all-features` is already an assertion
//! that every one of those 44 findings survived. This file covers the part
//! that pinning cannot: that no *new* finding appeared under a code those
//! expectations never mentioned.

use std::path::{Path, PathBuf};

use conform_okf::{Bundle, codes, lint_bundle, validate_bundle};

mod support;

use support::fixture;

/// Every bundle this crate's tests have, vendored and homemade.
///
/// The homemade one is included on purpose: it is the bundle whose code is
/// broken, so it is the bundle where a leak from the syntax rules into the
/// other two checks would actually show up. Asserting the property only over
/// bundles with nothing to find would be asserting nothing.
fn every_bundle() -> Vec<PathBuf> {
    vec![
        fixture("acme_retail"),
        fixture("ga4"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/syntax/broken"),
    ]
}

/// The codes only `OkfSyntax` may raise.
const SYNTAX_CODES: [&str; 2] = [codes::CODE_BLOCK_SYNTAX, codes::COMPUTATION_CODE_SYNTAX];

#[test]
fn neither_of_the_other_two_checks_ever_raises_a_syntax_code() {
    let mut faults = Vec::new();

    for root in every_bundle() {
        let bundle = Bundle::load(&root).expect("a test fixture loads");
        let name = root.display().to_string();

        for (check, report) in [
            ("validate", validate_bundle(&bundle).into_report()),
            ("lint", lint_bundle(&bundle).into_report()),
        ] {
            for diagnostic in &report {
                if SYNTAX_CODES.contains(&diagnostic.code.as_str()) {
                    faults.push(format!(
                        "{name}: {check} raised {}, which belongs to OkfSyntax; the syntax rules \
                         have leaked into a check that is supposed to be the same in every build",
                        diagnostic.code,
                    ));
                }
            }
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

/// The codes exist whether or not the check that raises them does.
///
/// A code is an identifier a consumer writes into a suppression list or a CI
/// annotation. One that appears and disappears with a build flag is not an
/// identifier, and a downstream file naming it would fail to compile against a
/// build that happened not to enable the feature.
///
/// This is a compile-time assertion first — the constants are named in a file
/// that builds with no features — and a value assertion second.
#[test]
fn the_syntax_codes_are_declared_unconditionally() {
    assert_eq!(codes::CODE_BLOCK_SYNTAX, "OKF007");
    assert_eq!(codes::COMPUTATION_CODE_SYNTAX, "OKF310");
}

/// And the assertion above is not vacuous: with the feature on, those codes
/// really are raised over the bundle it was run against.
///
/// Without this, `neither_of_the_other_two_checks_ever_raises_a_syntax_code`
/// would pass in a world where nothing raises them at all — which is exactly
/// the state a broken wiring would leave behind.
#[cfg(feature = "syntax")]
#[test]
fn the_check_that_may_raise_them_does() {
    use conform_core::Validator;
    use conform_okf::OkfSyntax;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/syntax/broken");
    let bundle = Bundle::load(&root).expect("the broken fixture loads");
    let report = OkfSyntax.validate(&bundle);

    let raised: Vec<&str> = report
        .iter()
        .map(|d| d.code.as_str())
        .filter(|code| SYNTAX_CODES.contains(code))
        .collect();

    assert!(
        !raised.is_empty(),
        "the broken fixture produced no syntax finding, so the assertion that the other two \
         checks never produce one is measuring nothing",
    );
}
