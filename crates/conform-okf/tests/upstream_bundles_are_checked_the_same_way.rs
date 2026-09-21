//! Check bundles **somebody else wrote**.
//!
//! A checker tested solely against documents its own project produced will
//! happily agree with itself about a format it also invents. The fixtures here
//! are two of the four bundles published in the specification's own
//! repository — see `tests/fixtures/okf-upstream/PROVENANCE.md` for their
//! commit, licence and the trimming applied. They are the closest thing there
//! is to an authoritative answer about what a conformant v0.2 bundle looks
//! like, because the people who wrote the specification wrote them.
//!
//! # Why the whole report is pinned, and not counts
//!
//! Each expectation below is the complete rendered report, in order. Counts
//! alone pass for a checker that fires the right number of rules on the wrong
//! documents, and a filtered subset passes for one that also emits something
//! nobody asserted on. The cost of pinning the whole thing is that an
//! intentional change to any message edits a test; that is the intended cost.
//!
//! # What these values are
//!
//! They are **not** this crate's output written down after the fact. Every
//! line below was produced by compiling the implementation this crate was
//! migrated from — `rto-render`'s `okf::conform`, verbatim apart from its two
//! `super::` couplings — and running it over these same bytes. The two
//! implementations were then diffed finding by finding across four runs, and
//! agreed on all 44: same severity, same rule, same document, same message,
//! same order. These are the *old* numbers.

mod support;

use conform_core::{GatePolicy, Severity};
use conform_okf::{Check, lint_report, validate_report};
use support::{fixture, render};

/// Four warnings and nothing else, over nine concepts. Two of them are the
/// bundle's own `attester.resource` pointing at `attesters/sql_equality.py` —
/// a Python file that exists upstream and was dropped when these fixtures were
/// trimmed to markdown, so the finding is true of the vendored corpus and is
/// the one place the trimming is visible in a report.
const ACME_RETAIL_CONFORMANCE: &str = "\
warning OKF401 computations/gross-margin-period.md: `attester.resource` names `attesters/sql_equality.py`, which the bundle does not contain
warning OKF401 computations/revenue-ytd.md: `attester.resource` names `attesters/sql_equality.py`, which the bundle does not contain
warning OKF403 metrics/gross-margin.md: links to deprecated concept `metrics/gross-margin-legacy`
warning OKF403 policies/margin-standard.md: links to deprecated concept `metrics/gross-margin-legacy`";

const ACME_RETAIL_HYGIENE: &str = "\
warning OKFL03 computations/gross-margin-period.md: multiple top-level `#` headings found (heading `Notes on the COGS composition` at line 44)
warning OKFL03 computations/gross-margin-period.md: multiple top-level `#` headings found (heading `Freshness` at line 50)
warning OKFL03 computations/revenue-ytd.md: multiple top-level `#` headings found (heading `What the attester checks` at line 27)
warning OKFL03 computations/revenue-ytd.md: multiple top-level `#` headings found (heading `Freshness` at line 36)
warning OKFL05 computations/revenue-ytd.md: source `orders-table` is declared in frontmatter but never cited with footnote `[^orders-table]`
warning OKFL03 metrics/gross-margin-legacy.md: multiple top-level `#` headings found (heading `Legacy definition (for reproducibility only)` at line 7)
warning OKFL03 metrics/gross-margin-legacy.md: multiple top-level `#` headings found (heading `Why no attested computation` at line 17)
warning OKFL03 metrics/gross-margin.md: multiple top-level `#` headings found (heading `What changed in FY2026` at line 13)
warning OKFL03 metrics/gross-margin.md: multiple top-level `#` headings found (heading `Trust and freshness` at line 19)
warning OKFL03 metrics/revenue.md: multiple top-level `#` headings found (heading `Reporting cuts` at line 7)
warning OKFL03 metrics/revenue.md: multiple top-level `#` headings found (heading `Trust and freshness` at line 12)
warning OKFL03 policies/margin-standard.md: multiple top-level `#` headings found (heading `Cited by` at line 36)
warning OKFL03 policies/revenue-recognition.md: multiple top-level `#` headings found (heading `Cited by` at line 31)
warning OKFL03 tables/orders.md: multiple top-level `#` headings found (heading `Notes for consumers` at line 17)
info OKFL02 computations/gross-margin-period.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL02 computations/revenue-ytd.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL06 computations/revenue-ytd.md: author `team:data-platform` in `sources.author` does not follow §7's `human:<id>`, `process:<id>` or `<producer>/<version>` convention
info OKFL02 metrics/gross-margin-legacy.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL02 metrics/gross-margin.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL02 metrics/revenue.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL02 policies/margin-standard.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL02 policies/revenue-recognition.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL02 skills/run-on-bq.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL11 skills/run-on-bq.md: no `verified` events; trust tier is `unverified`
info OKFL02 tables/orders.md: frontmatter keys are not in canonical order (§5's reading order)
info OKFL06 tables/orders.md: author `team:data-platform` in `sources.author` does not follow §7's `human:<id>`, `process:<id>` or `<producer>/<version>` convention";

const GA4_CONFORMANCE: &str = "\
warning OKF407 index.md: index lists `datasets/index.md`, which no longer exists; a reader following the listing lands on nothing
warning OKF407 index.md: index lists `references/index.md`, which no longer exists; a reader following the listing lands on nothing
info OKF402 tables/events_.md: link `../references/metrics/purchasers.md` names `references/metrics/purchasers`, which the bundle does not contain; §6 tells a consumer to tolerate this
info OKF402 tables/events_.md: link `../references/metrics/n_day_active_users.md` names `references/metrics/n_day_active_users`, which the bundle does not contain; §6 tells a consumer to tolerate this
info OKF402 tables/events_.md: link `../references/metrics/n_day_inactive_users.md` names `references/metrics/n_day_inactive_users`, which the bundle does not contain; §6 tells a consumer to tolerate this
info OKF402 tables/events_.md: link `../references/metrics/frequently_active_users.md` names `references/metrics/frequently_active_users`, which the bundle does not contain; §6 tells a consumer to tolerate this
info OKF402 tables/events_.md: link `../references/metrics/highly_active_users.md` names `references/metrics/highly_active_users`, which the bundle does not contain; §6 tells a consumer to tolerate this
info OKF402 tables/events_.md: link `../references/metrics/acquired_users.md` names `references/metrics/acquired_users`, which the bundle does not contain; §6 tells a consumer to tolerate this
info OKF402 tables/events_.md: link `../references/metrics/google_acquired_cohorts.md` names `references/metrics/google_acquired_cohorts`, which the bundle does not contain; §6 tells a consumer to tolerate this";

const GA4_HYGIENE: &str = "\
warning OKFL03 tables/events_.md: multiple top-level `#` headings found (heading `Common query patterns` at line 75)
warning OKFL03 tables/events_.md: heading level skipped: `1. Count events and active users by event name` jumps from h1 to h3
warning OKFL03 tables/events_.md: multiple top-level `#` headings found (heading `Metrics` at line 136)
warning OKFL05 tables/events_.md: source `sample_queries` is declared in frontmatter but never cited with footnote `[^sample_queries]`
info OKFL11 tables/events_.md: no `verified` events; trust tier is `unverified`";

/// `acme_retail` uses flow mappings throughout, which is the form the
/// specification's own examples use, and it is the only upstream bundle
/// exercising `type: Attested Computation`, `stale_after`,
/// `status: deprecated` and per-source credibility signals.
#[test]
fn a_published_bundle_conforms_and_is_not_gated() {
    let report = validate_report(fixture("acme_retail")).expect("load acme_retail");

    assert_eq!(report.check(), Check::Validate);
    assert_eq!(
        report.concepts(),
        9,
        "acme_retail holds nine concept documents; a report with no findings \
         over a bundle with no concepts is not a clean bill of health, which \
         is why the count is asserted alongside them"
    );
    assert_eq!(render(&report), ACME_RETAIL_CONFORMANCE);

    // The specification's own corpus must pass the gate. A conformance check
    // that fails on the documents its authors published is a check nobody can
    // use, and it is the one outcome that would make this crate worthless.
    assert_eq!(report.report().count(Severity::Error), 0);
    assert!(report.passed());
    assert!(!report.report().should_gate(GatePolicy::default()));
}

/// The hygiene rules over the same bundle: twenty-six opinions about a bundle
/// conformance is content with, and not one of them an error.
#[test]
fn the_same_published_bundle_has_hygiene_findings_and_still_cannot_gate() {
    let report = lint_report(fixture("acme_retail")).expect("load acme_retail");

    assert_eq!(report.check(), Check::Lint);
    assert_eq!(report.concepts(), 9);
    assert_eq!(render(&report), ACME_RETAIL_HYGIENE);

    assert_eq!(report.report().count(Severity::Error), 0);
    assert!(
        !report
            .report()
            .should_gate(conform_okf::OkfHygiene.gate_policy())
    );
}

/// `ga4` is machine-serialised in `PyYAML`'s default style: block sequences
/// whose items sit at the parent key's own indentation, and folded multi-line
/// scalars. It is also the bundle that was trimmed, so its index lists two
/// directories that are not vendored — which is what `OKF407` reports, and it
/// is correct about the corpus it was given.
#[test]
fn a_machine_serialised_bundle_is_read_the_same_way() {
    let report = validate_report(fixture("ga4")).expect("load ga4");

    assert_eq!(report.concepts(), 1);
    assert_eq!(render(&report), GA4_CONFORMANCE);
    assert!(report.passed());
}

#[test]
fn a_machine_serialised_bundle_has_the_hygiene_findings_it_had_before() {
    let report = lint_report(fixture("ga4")).expect("load ga4");

    assert_eq!(report.concepts(), 1);
    assert_eq!(render(&report), GA4_HYGIENE);
}

/// The counts, stated separately from the reports above.
///
/// Not redundant: the assertions above would also pass if `render` silently
/// dropped diagnostics on both sides of the comparison, because it is used to
/// build one side and the other is a literal derived from it. This reads the
/// report through a different accessor and asserts the totals the migration
/// was measured against.
#[test]
fn the_totals_are_the_ones_the_migration_was_measured_against() {
    let counts = |report: &conform_okf::BundleReport| {
        (
            report.report().len(),
            report.report().count(Severity::Error),
            report.report().count(Severity::Warning),
            report.report().count(Severity::Info),
        )
    };

    assert_eq!(
        counts(&validate_report(fixture("acme_retail")).expect("load")),
        (4, 0, 4, 0)
    );
    assert_eq!(
        counts(&lint_report(fixture("acme_retail")).expect("load")),
        (26, 0, 14, 12)
    );
    assert_eq!(
        counts(&validate_report(fixture("ga4")).expect("load")),
        (9, 0, 2, 7)
    );
    assert_eq!(
        counts(&lint_report(fixture("ga4")).expect("load")),
        (5, 0, 4, 1)
    );
}

/// Every diagnostic names the specification it was raised under, and the
/// reference resolves to the `okf` entry in the repository's registry.
#[test]
fn every_finding_says_which_specification_said_so() {
    for bundle in ["acme_retail", "ga4"] {
        for report in [
            validate_report(fixture(bundle)).expect("load"),
            lint_report(fixture(bundle)).expect("load"),
        ] {
            for diagnostic in report.report() {
                let spec = diagnostic
                    .spec_ref
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} carries no spec_ref", diagnostic.code));
                assert_eq!(spec.id, conform_okf::SPEC_ID);
                assert_eq!(spec.version.as_deref(), Some(conform_okf::OKF_VERSION));
            }
        }
    }
}

/// A bundle root that is not a bundle is a load failure, reported as
/// diagnostics rather than as a string — and it is *not* an empty report,
/// which is the outcome that would read as "checked, nothing wrong".
#[test]
fn a_path_that_is_not_a_bundle_is_a_diagnostic_and_not_an_empty_report() {
    let error = validate_report(fixture("no-such-bundle")).expect_err("not a bundle");

    assert_eq!(error.report().len(), 1);
    let diagnostic = &error.report().diagnostics()[0];
    assert_eq!(
        diagnostic.code.as_str(),
        conform_okf::codes::BUNDLE_UNREADABLE
    );
    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(error.report().should_gate(GatePolicy::default()));
}
