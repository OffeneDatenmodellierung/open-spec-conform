//! A feature that is paid for must arrive.
//!
//! # The defect this exists to prevent, which is not hypothetical
//!
//! This crate's `syntax-sql` and `syntax-grammars` features forward to
//! `conform-okf`, and the merge that puts `OkfSyntax`'s findings into a run is
//! behind this crate's own `syntax` cfg. The first version of that manifest
//! read:
//!
//! ```toml
//! syntax     = ["conform-okf/syntax"]
//! syntax-sql = ["conform-okf/syntax-sql"]
//! ```
//!
//! which is wrong in a way nothing complains about. `--features syntax-sql`
//! compiled `conform-okf` **with** `sqlparser`, linked it in, and then compiled
//! the only call to it out of this binary. The build paid ~17 crates, two of
//! them assembling, and reported exactly what a default build reports. Every
//! test passed. It was caught by running the binary and reading the output,
//! which is the only thing that would have caught it.
//!
//! So there are two checks here, and they fail in different places on purpose:
//! one reads the manifest and would have failed on the bad version above, and
//! one runs the engine and fails if the wiring behind the cfg is ever removed.

use std::fs;
use std::path::Path;

/// This crate's own manifest.
fn manifest() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every `name = [...]` line in `[features]`, as `(name, the list as written)`.
fn features() -> Vec<(String, String)> {
    manifest()
        .lines()
        .skip_while(|line| line.trim() != "[features]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, rest) = line.split_once('=')?;
            Some((name.trim().to_owned(), rest.trim().to_owned()))
        })
        .collect()
}

/// A scanner that finds nothing would make both assertions below vacuous.
#[test]
fn the_scan_reaches_the_feature_table() {
    let features = features();
    assert!(
        features.iter().any(|(name, _)| name == "syntax"),
        "the feature table scan did not find `syntax`; it found {:?}",
        features.iter().map(|(n, _)| n).collect::<Vec<_>>(),
    );
}

/// Every tier that forwards a parser also turns on the cfg that reads it.
///
/// `syntax` is what `engine::bundle_report` compiles against. A tier that
/// forwards `conform-okf/syntax-sql` without also naming `syntax` produces a
/// binary that carries a SQL parser it never calls.
#[test]
fn every_forwarding_feature_also_enables_the_cfg_that_reads_it() {
    let mut faults = Vec::new();

    for (name, list) in features() {
        if name == "syntax" || !name.starts_with("syntax") {
            continue;
        }
        if !list.contains("\"syntax\"") {
            faults.push(format!(
                "`{name} = {list}` forwards a parser to conform-okf without naming `syntax`, so \
                 a build with it pays for the parser and compiles out the call to it",
            ));
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

/// And with the cfg on, a run over a bundle with broken code really does carry
/// the finding.
///
/// The manifest check above is about the wiring being *declared*; this is
/// about it being *connected*. Removing the `report.merge(...)` line in
/// `engine::bundle_report` would leave the manifest check green.
#[cfg(feature = "syntax")]
#[test]
fn a_syntax_finding_reaches_the_run() {
    use conform_cli::engine::{self, Request};
    use conform_cli::model::Command;
    use conform_core::GatePolicy;

    // `conform-okf`'s fixture, reached across the workspace: this crate has no
    // OKF bundle of its own, and vendoring a second copy of one would be the
    // drift `vendored_bytes_are_packaged_not_forked` exists to prevent.
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("conform-okf/tests/fixtures/syntax/broken");
    assert!(bundle.is_dir(), "{} is not there", bundle.display());

    let run = engine::run(&Request {
        command: Command::Validate,
        paths: vec![bundle],
        spec: Some("okf".to_owned()),
        policy: GatePolicy::default(),
        registry: None,
    });

    let codes: Vec<&str> = run
        .documents
        .iter()
        .flat_map(|document| document.report.iter())
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();

    assert!(
        codes.contains(&"OKF007"),
        "no OKF007 in a run over a bundle with a broken JSON block; the syntax report is not \
         reaching the run. Codes seen: {codes:?}",
    );
    assert!(
        !cfg!(feature = "syntax-sql") || codes.contains(&"OKF310"),
        "this build has SQL compiled in and did not report the `# Computation` block that does \
         not parse. Codes seen: {codes:?}",
    );
}
