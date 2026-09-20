//! The `validate` / `lint` split, and what it is *made of* after the
//! migration.
//!
//! Before this crate existed the two checks were one function each, returning
//! one report type, and the difference between them was that a caller was
//! trusted to know which of the two totals to look at. That is a convention,
//! and a convention is a thing somebody eventually gets wrong at four in the
//! afternoon.
//!
//! Here the difference is `conform-core`'s report-versus-gate separation, and
//! it holds in two independent ways. The rules themselves cannot produce an
//! error from hygiene, and the policy hygiene asks to be read under cannot
//! fail on anything. Each of the tests below would still pass if the other
//! property were broken, which is the point of asserting both.

mod support;

use conform_core::{GatePolicy, GateVerdict, Severity, Validator};
use conform_okf::{Bundle, OkfConformance, OkfHygiene, lint_report, validate_report};
use support::fixture;

/// A bundle with a document that is not OKF at all.
fn unparseable_bundle(dir: &std::path::Path) -> Bundle {
    std::fs::create_dir_all(dir).expect("create bundle dir");
    std::fs::write(dir.join("broken.md"), "---\ntype: [\n---\n").expect("write broken document");
    std::fs::write(
        dir.join("index.md"),
        "---\ntitle: Broken\n---\n\n# Broken\n\n- [Broken](broken.md)\n",
    )
    .expect("write index");
    Bundle::load(dir).expect("a directory is still a bundle")
}

/// Hygiene has no way to raise an error over any bundle, including one that
/// conformance fails outright.
#[test]
fn hygiene_cannot_produce_an_error_even_over_a_bundle_that_is_not_okf() {
    let dir = std::env::temp_dir().join("conform-okf-broken-bundle");
    let bundle = unparseable_bundle(&dir);

    let conformance = OkfConformance.validate(&bundle);
    assert_eq!(
        conformance.count(Severity::Error),
        1,
        "a document that did not parse is a conformance error"
    );

    let hygiene = OkfHygiene.validate(&bundle);
    assert_eq!(
        hygiene.count(Severity::Error),
        0,
        "the same bundle, asked the hygiene question, yields no errors: the \
         rules cannot raise one"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// And the policy hygiene names would not fail on one if a rule ever did.
#[test]
fn hygiene_would_not_gate_even_on_an_error() {
    let mut report = conform_core::ConformanceReport::new();
    report.push(conform_core::Diagnostic::error(
        "OKF001",
        conform_core::Location::document("invented.md"),
        "a hypothetical error, to prove the policy and not the rules",
    ));

    assert_eq!(OkfHygiene.gate_policy(), GatePolicy::report_only());
    assert!(!report.should_gate(OkfHygiene.gate_policy()));
    assert_eq!(
        report.gate(OkfHygiene.gate_policy()),
        GateVerdict::Passed {
            worst: Some(Severity::Error)
        },
        "passed, and the worst severity is still on the record — `passed` and \
         `nothing was found` are different statements"
    );

    // The contrast, on the same report.
    assert!(report.should_gate(OkfConformance.gate_policy()));
}

/// Conformance gates on errors and on nothing softer. Warnings are reported
/// and do not fail: §11 tells a consumer not to reject a document over a
/// soft-guidance deviation, and the specification's own corpus carries
/// warnings.
#[test]
fn conformance_gates_on_errors_and_not_on_warnings() {
    let report = validate_report(fixture("acme_retail")).expect("load acme_retail");

    assert!(report.report().count(Severity::Warning) > 0);
    assert_eq!(report.report().count(Severity::Error), 0);

    assert!(!report.report().should_gate(OkfConformance.gate_policy()));
    assert!(report.passed());

    // A caller who wants a stricter build gets one by naming a policy, not by
    // this crate deciding for them.
    assert!(
        report
            .report()
            .should_gate(GatePolicy::warnings_as_errors())
    );
}

/// `validate` reports and cannot fail; `check` gates against a policy the
/// caller names. The trait's shape is what makes that true, so it is asserted
/// through the trait.
#[test]
fn validate_reports_and_check_gates() {
    let bundle = Bundle::load(fixture("ga4")).expect("load ga4");

    let reported = OkfConformance.validate(&bundle);
    assert!(!reported.is_empty(), "ga4 has findings worth having");

    assert_eq!(
        OkfConformance.check(&bundle, GatePolicy::report_only()),
        GateVerdict::Passed {
            worst: Some(Severity::Warning)
        }
    );
    assert_eq!(
        OkfConformance.check(&bundle, GatePolicy::default()),
        GateVerdict::Passed {
            worst: Some(Severity::Warning)
        }
    );
    assert_eq!(
        OkfConformance.check(&bundle, GatePolicy::warnings_as_errors()),
        GateVerdict::Gated {
            worst: Severity::Warning
        },
        "the same report, three policies, three separately-decided verdicts"
    );
}

/// The count of concepts examined is part of the answer, not decoration.
///
/// A report with no findings over a bundle with no concepts reads exactly like
/// a clean bill of health, and the two are not the same statement.
#[test]
fn an_empty_bundle_is_not_a_clean_bill_of_health() {
    let dir = std::env::temp_dir().join("conform-okf-empty-bundle");
    std::fs::create_dir_all(&dir).expect("create empty bundle dir");

    let report = validate_report(&dir).expect("an empty directory is still a bundle");

    assert!(report.report().is_empty());
    assert!(report.passed());
    assert_eq!(
        report.concepts(),
        0,
        "nothing was found because nothing was examined, and the report says so"
    );

    let hygiene = lint_report(&dir).expect("an empty directory is still a bundle");
    assert_eq!(hygiene.concepts(), 0);

    let _ = std::fs::remove_dir_all(&dir);
}
