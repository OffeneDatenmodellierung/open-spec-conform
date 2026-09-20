//! Driving whole invocations without a process.
//!
//! Every test in this crate runs the binary the way a user does — same
//! argument parsing, same engine, same renderers — by calling
//! [`conform_cli::execute`] with its output streams in hand. No `Command`, no
//! `target/debug` path, no temporary shell. What a test reads is byte-for-byte
//! what a user would have seen on stdout, and the integer it gets back is the
//! process exit code.

// Each integration test compiles this module separately, so a helper used by
// one of them is dead code in the others. The alternative — a helper per test
// file — would let the tests drift apart on how they invoke the binary, which
// is the one thing they must agree on.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The workspace root — the directory `specs.toml` lives in.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-cli should sit two levels below the workspace root")
        .to_path_buf()
}

/// The repository's real registry.
///
/// Passed explicitly with `--registry` rather than relying on the search from
/// the current directory: a test that depended on where `cargo` happened to be
/// invoked from would be a test that passes for the wrong reason.
pub fn specs_toml() -> PathBuf {
    workspace_root().join("specs.toml")
}

/// A path inside the workspace, as a string suitable for an argument.
pub fn in_workspace(relative: &str) -> String {
    workspace_root().join(relative).display().to_string()
}

/// What one invocation produced.
pub struct Output {
    /// Everything written to stdout.
    pub stdout: String,
    /// Everything written to stderr.
    pub stderr: String,
    /// The process exit code.
    pub code: i32,
}

/// Run `conform` with these arguments, against this repository's registry.
///
/// `--registry` is appended, so no caller has to remember to.
pub fn conform(args: &[&str]) -> Output {
    let registry = specs_toml().display().to_string();
    let mut command_line: Vec<String> = std::iter::once("conform".to_owned())
        .chain(args.iter().map(|arg| (*arg).to_owned()))
        .collect();
    command_line.push("--registry".to_owned());
    command_line.push(registry);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = conform_cli::execute(command_line, &mut stdout, &mut stderr);

    Output {
        stdout: String::from_utf8(stdout).expect("the human renderer emits UTF-8"),
        stderr: String::from_utf8(stderr).expect("errors are UTF-8"),
        code,
    }
}

/// Run `conform`, and parse its `--json` envelope.
///
/// # Panics
///
/// If the output is not JSON, which would be a defect in the renderer rather
/// than a finding about a document.
pub fn conform_json(args: &[&str]) -> (serde_json::Value, i32) {
    let mut command_line: Vec<&str> = args.to_vec();
    command_line.push("--json");
    let output = conform(&command_line);
    let value = serde_json::from_str(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "`--json` did not produce JSON: {error}\n---\n{}\n---",
            output.stdout
        )
    });
    (value, output.code)
}

/// A directory this test may write into, unique to the calling test.
///
/// `CARGO_TARGET_TMPDIR` is cargo's own scratch directory for integration
/// tests, so nothing here touches the repository or the system temporary
/// directory.
///
/// # Panics
///
/// If the directory cannot be created, which is a broken environment rather
/// than a failing assertion.
pub fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", path.display()));
    path
}

/// Write a file into a scratch directory and return its path as a string.
///
/// # Panics
///
/// If the file cannot be written.
pub fn write(directory: &Path, name: &str, contents: &str) -> String {
    let path = directory.join(name);
    std::fs::write(&path, contents)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    path.display().to_string()
}
