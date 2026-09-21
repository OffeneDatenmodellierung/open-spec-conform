//! The journey a person who ran `cargo install conform-cli` actually takes.
//!
//! # Why this drives the binary instead of calling `engine::run`
//!
//! The defect this test exists to prevent was not visible from inside the
//! library. `engine::run` was always correct; what was wrong was what happened
//! when the process started somewhere with no `specs.toml` above it, which is
//! where every installed copy of this tool starts. A unit test calling `run`
//! from the crate directory can never be in that situation, because the crate
//! directory is inside this repository.
//!
//! So these tests launch the real executable — `CARGO_BIN_EXE_conform`, the one
//! cargo just built — in a directory of their own choosing, and read its stdout
//! and its exit code. That is the only arrangement in which "an installed
//! binary works" is a thing a test can actually say.
//!
//! # Why the registry line is asserted on every time
//!
//! An embedded catalogue answering silently for a repository the reader
//! believed they were checking is a provenance failure of exactly the kind this
//! project exists to prevent. Which registry answered is therefore part of the
//! output contract, not a nicety, and each test below asserts it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

/// The executable cargo built for this test run.
const CONFORM: &str = env!("CARGO_BIN_EXE_conform");

/// This repository's root.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-cli sits two levels below the repository root")
        .to_path_buf()
}

/// A real ODCS contract, written here rather than borrowed from a sibling
/// crate's fixtures.
///
/// `conform-cli`'s tarball contains `conform-cli`'s files and nobody else's, so
/// a test that reached into `conform-odcs/tests/fixtures` would pass here and
/// fail for anyone who unpacked the published crate — which is the whole class
/// of defect this file is about.
const CONTRACT: &str = "\
apiVersion: v3.0.2
kind: DataContract
id: 53581432-6c55-4ba2-a65f-72344a91553a
version: 1.0.0
status: active
name: orders
";

/// A directory outside this repository, with no `specs.toml` above it.
///
/// Returns [`None`] if the temporary directory turns out to sit under one —
/// which would make these tests measure nothing. That is reported as a failure
/// at the call site rather than silently skipped.
fn elsewhere(label: &str) -> Option<PathBuf> {
    let dir = env::temp_dir().join(format!("conform-installed-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a temporary directory can be created");

    let canonical = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
    if canonical
        .ancestors()
        .any(|d| d.join("specs.toml").is_file())
    {
        return None;
    }
    fs::write(dir.join("contract.yaml"), CONTRACT).expect("the fixture can be written");
    Some(dir)
}

/// Run `conform` somewhere, and return its stdout and exit code.
fn conform(cwd: &Path, args: &[&str]) -> (String, i32) {
    let output = Command::new(CONFORM)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("could not run {CONFORM}: {error}"));
    let stdout = String::from_utf8(output.stdout).expect("conform writes UTF-8");
    (stdout, output.status.code().unwrap_or(-1))
}

/// The `registry:` line, which every run prints second.
fn registry_line(stdout: &str) -> &str {
    stdout
        .lines()
        .find(|line| line.starts_with("registry:"))
        .unwrap_or_else(|| panic!("every run prints a `registry:` line; got:\n{stdout}"))
}

#[test]
fn with_no_registry_on_disk_it_validates_against_the_embedded_one() {
    let Some(dir) = elsewhere("embedded") else {
        panic!(
            "the temporary directory sits under a `specs.toml`, so this test cannot tell an \
             embedded fallback from a discovered registry"
        );
    };

    let (stdout, code) = conform(&dir, &["validate", "contract.yaml"]);

    // The defect: this used to be `CLI100`, exit 2, and no verdict at all.
    assert!(
        !stdout.contains("CLI100"),
        "an installed binary must not report `no specs.toml`; it carries one:\n{stdout}",
    );
    assert_eq!(
        code, 0,
        "a clean contract should report and exit 0, not refuse to run:\n{stdout}",
    );

    // A real verdict, not an empty run. `ODCS904` is the adapter's own note
    // saying which schema it validated against, so its presence means a schema
    // was compiled and used.
    assert!(
        stdout.contains("ODCS904"),
        "the run should carry the adapter's `validated against` note:\n{stdout}",
    );
    assert!(
        stdout.contains("0 error(s)"),
        "this contract is conformant and should report no errors:\n{stdout}",
    );

    // And it says where the catalogue came from.
    let line = registry_line(&stdout);
    assert!(
        line.contains("embedded in conform-cli"),
        "the run must name the embedded catalogue and the crate version it was pinned at, got: \
         {line}",
    );

    // The provenance sentence must not claim more than an embedded check can.
    assert!(
        stdout.contains("says nothing about any `specs.toml` on disk"),
        "an embedded verdict must say what it cannot speak for:\n{stdout}",
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn inside_a_checkout_the_on_disk_registry_wins() {
    let root = repository_root();
    let (stdout, code) = conform(&root, &["validate", "crates/conform-odcs/tests/fixtures"]);

    let line = registry_line(&stdout);
    assert!(
        line.contains("found by searching upward"),
        "run from inside a checkout, the discovered registry must answer, got: {line}",
    );
    assert!(
        line.contains("specs.toml"),
        "the discovered registry line must name the file it found, got: {line}",
    );
    assert!(
        !line.contains("embedded"),
        "the embedded catalogue must never answer for a checkout the reader is standing in, \
         got: {line}",
    );
    assert!(
        code == 0 || code == 1,
        "the run should have happened; exit {code} means it could not:\n{stdout}",
    );
}

#[test]
fn a_subdirectory_of_a_checkout_still_finds_the_on_disk_registry() {
    // The regression that would be easiest to miss: the search walks upward, so
    // a binary run three directories deep must still reach the repository's
    // registry rather than falling through to its own.
    let deep = repository_root().join("crates/conform-cli/src");
    let (stdout, _) = conform(&deep, &["registry", "list"]);
    let line = registry_line(&stdout);
    assert!(
        line.contains("found by searching upward") && !line.contains("embedded"),
        "from a subdirectory the on-disk registry must still win, got: {line}",
    );
}

#[test]
fn an_explicit_registry_overrides_both() {
    let Some(dir) = elsewhere("explicit") else {
        panic!("the temporary directory sits under a `specs.toml`");
    };
    let specs = repository_root().join("specs.toml");

    let (stdout, code) = conform(
        &dir,
        &[
            "validate",
            "contract.yaml",
            "--registry",
            &specs.display().to_string(),
        ],
    );

    let line = registry_line(&stdout);
    assert!(
        line.contains("(--registry)"),
        "an explicit registry must be named as explicit, got: {line}",
    );
    assert!(
        !line.contains("embedded"),
        "`--registry` must beat the embedded catalogue, got: {line}",
    );
    assert_eq!(code, 0, "the run should have succeeded:\n{stdout}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_json_envelope_says_which_registry_answered() {
    let Some(dir) = elsewhere("json") else {
        panic!("the temporary directory sits under a `specs.toml`");
    };

    let (stdout, _) = conform(&dir, &["validate", "contract.yaml", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("`--json` should emit JSON: {e}\n{stdout}"));

    assert_eq!(
        value["registry_origin"]["kind"], "embedded",
        "a consumer gating a pipeline must be able to branch on the origin without parsing a \
         sentence:\n{stdout}",
    );
    assert!(
        value["registry_origin"]["path"].is_null(),
        "there is no file behind an embedded catalogue, and a plausible path that resolves to \
         nothing is worse than an honest null:\n{stdout}",
    );
    assert_eq!(
        value["schema_version"], 1,
        "`registry_origin` is additive, so the envelope version does not move",
    );

    let _ = fs::remove_dir_all(&dir);
}
