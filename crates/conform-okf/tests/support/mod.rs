//! Shared by the integration tests: where the fixtures are, and how a report
//! is rendered so an assertion failure is readable.

// Each integration test compiles this module separately, so a helper used by
// one of them is dead code in the others. The alternative — a helper per test
// file — would let the tests drift apart on how they locate the fixtures and
// how they render a report, which are the two things they must agree on.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use conform_core::ConformanceReport;
use conform_okf::BundleReport;

/// The directory holding the vendored upstream bundles.
pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/okf-upstream")
}

/// One vendored upstream bundle.
pub fn fixture(bundle: &str) -> PathBuf {
    fixture_root().join(bundle)
}

/// Every diagnostic as `severity code document: message`, one per line.
///
/// Compared as one string rather than field by field, because the thing under
/// test is the *whole* report: a rule that fires with the right message on the
/// wrong document, or in the wrong order, is a defect that a per-field
/// assertion over a filtered subset would not see.
pub fn render(report: &BundleReport) -> String {
    render_report(report.report())
}

/// The same rendering, for a check that returns a bare report rather than a
/// [`BundleReport`].
///
/// One renderer and not two: the pinned expectations in these files are
/// compared as text, so two spellings of "a diagnostic as a line" would make
/// two test files disagree about what they are pinning.
pub fn render_report(report: &ConformanceReport) -> String {
    report
        .iter()
        .map(|d| {
            format!(
                "{} {} {}: {}",
                d.severity, d.code, d.location.document, d.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
