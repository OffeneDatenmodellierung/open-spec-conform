//! What `OkfSyntax` actually says, on code that is right and on code that is
//! wrong.
//!
//! # Two fixtures, because one of them cannot prove it
//!
//! Every tagged code block in the vendored upstream corpus parses. That is a
//! useful fact — it is what makes this check usable against third-party
//! bundles at all, and the direction of failure that would make it harmful is
//! a *false* finding, not a missed one — but a checker that never fires and a
//! checker that is switched off produce the same empty report.
//!
//! So `tests/fixtures/syntax/broken/` is a bundle this repository wrote whose
//! blocks are wrong on purpose, and the two are asserted together: the
//! upstream corpus must stay silent *while containing blocks this build can
//! actually read*, and the broken bundle must produce exactly the findings
//! that were planted in it.
//!
//! # Why the expectations move with the features
//!
//! Every parser in `conform-okf-syntax` is optional, so the answer to "was
//! this block checked?" is a property of the build. A JSON block is caught by
//! `syntax` alone; a Python block needs `syntax-grammars`; the SQL in the
//! `# Computation` block needs `syntax-sql`. Asserting a single expectation in
//! every configuration would mean asserting the weakest one, which is how a
//! suite stays green over a build that checks nothing.

#![cfg(feature = "syntax")]

use std::path::{Path, PathBuf};

use conform_core::{Severity, Validator};
use conform_okf::conform_okf_syntax::{Language, extract_fenced_code_blocks, is_checkable};
use conform_okf::{Bundle, OkfSyntax, codes};

mod support;

use support::{fixture, render_report};

/// The bundle this repository wrote to be wrong. Not vendored, no digest, and
/// deliberately a sibling of `okf-upstream/` rather than inside it — see
/// `tests/fixtures/syntax/README.md`.
fn broken() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/syntax/broken")
}

/// One line per finding that this build is expected to produce, in report
/// order: warnings before information, bundle order within a severity.
fn expected() -> Vec<&'static str> {
    let mut lines = Vec::new();

    if cfg!(feature = "syntax-sql") {
        lines.push(
            "warning OKF310 computations/broken-sql.md: the `# Computation` block does not \
             parse as `sql`: sql parser error: Expected: an expression, found: EOF",
        );
    }

    // JSON needs no feature beyond `syntax` itself: `serde_json` is in the
    // core, so this line is the one that holds in every configuration here.
    lines.push(
        "info OKF007 notes/fenced-blocks.md: fenced `json` block opening at line 6 does not \
         parse: trailing comma at line 4 column 1 (line 4 of the block)",
    );

    if cfg!(feature = "syntax-grammars") {
        lines.push(
            "info OKF007 notes/fenced-blocks.md: fenced `python` block opening at line 18 does \
             not parse: unexpected input (line 1 of the block)",
        );
    }

    lines
}

#[test]
fn the_planted_findings_are_found_and_nothing_else_is() {
    let bundle = Bundle::load(broken()).expect("the broken fixture is still a loadable bundle");
    let report = OkfSyntax.validate(&bundle);

    assert_eq!(
        render_report(&report),
        expected().join("\n"),
        "the whole report is pinned, not a count: a rule that fires with the right message on \
         the wrong document would pass a count",
    );

    // The valid SQL block, the untagged block and the `mermaid` block are in
    // the same document as the two findings above, so their silence is not the
    // silence of a document nobody read.
    let quiet = ["SELECT COUNT(*)", "SELCT this is not anything", "graph TD"];
    let body = &bundle
        .concepts()
        .iter()
        .find(|c| c.path.ends_with("fenced-blocks.md"))
        .expect("the fixture's second concept")
        .document
        .body;
    for snippet in quiet {
        assert!(
            body.contains(snippet),
            "the fixture no longer contains {snippet:?}, so this test is no longer checking \
             that a block of that kind stays quiet",
        );
    }
}

/// Nothing here is an error, and nothing here gates.
///
/// Both halves, because either alone is satisfiable by accident: a check that
/// raised an error under a `report_only` policy would still not gate, and a
/// check that raised no error could still be read under a policy that failed
/// on warnings.
#[test]
fn broken_code_is_never_a_conformance_failure() {
    let bundle = Bundle::load(broken()).expect("the broken fixture is still a loadable bundle");
    let report = OkfSyntax.validate(&bundle);

    assert!(
        report.count(Severity::Error) == 0,
        "a bundle whose code does not parse is a bundle with a bug in it, not a document that \
         fails to be OKF",
    );
    assert!(!report.should_gate(OkfSyntax.gate_policy()));
}

/// The upstream corpus is silent, and silent about something.
///
/// The second half is the one that matters. Without it this test would pass on
/// a build whose every parser had been removed, on a corpus with no code in it,
/// or on a walk that visited no documents.
#[test]
fn the_upstream_corpus_stays_quiet_over_blocks_this_build_can_read() {
    let mut checkable_blocks = 0usize;

    for name in ["acme_retail", "ga4"] {
        let bundle = Bundle::load(fixture(name)).expect("a vendored bundle loads");
        let report = OkfSyntax.validate(&bundle);

        assert_eq!(
            render_report(&report),
            "",
            "{name} is somebody else's published bundle and its code parses; a finding here is \
             this crate being wrong about their document, which is the one failure mode that \
             would make the check unusable",
        );

        for concept in bundle.concepts() {
            checkable_blocks += extract_fenced_code_blocks(&concept.document.body)
                .iter()
                .filter_map(|block| block.language.as_deref())
                .filter(|tag| is_checkable(Language::from_tag(tag)))
                .count();
        }
    }

    // Five tagged SQL blocks across the two vendored bundles, two of which are
    // the Attested Computations. `syntax` alone can read none of them, so the
    // floor is only meaningful where SQL is compiled in.
    let floor = usize::from(cfg!(feature = "syntax-sql")) * 5;
    assert!(
        checkable_blocks >= floor,
        "this build read {checkable_blocks} blocks of the upstream corpus and should have read \
         at least {floor}; an empty report over nothing is not a clean bill of health",
    );
}

/// The codes are this crate's, and they are the two it documents.
#[test]
fn the_two_codes_are_the_two_checks() {
    assert_eq!(codes::COMPUTATION_CODE_SYNTAX, "OKF310");
    assert_eq!(codes::CODE_BLOCK_SYNTAX, "OKF007");
    assert_eq!(
        codes::upstream_rule(codes::CODE_BLOCK_SYNTAX),
        None,
        "these are conformance-range codes, not hygiene rules carried forward from upstream's \
         `L` namespace",
    );
}
