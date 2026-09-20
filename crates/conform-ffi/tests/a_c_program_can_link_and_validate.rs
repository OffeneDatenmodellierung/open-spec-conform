//! A real C compiler, a real linker, a real document.
//!
//! # What this catches that nothing else can
//!
//! Every other test in this crate is Rust calling Rust through an `extern "C"`
//! signature. That proves the logic and the pointer discipline, and it proves
//! nothing at all about the *header*, because the Rust caller never reads it.
//! A header that declares an argument the library does not take, a symbol that
//! was not exported, an enum whose values drifted — all three compile and link
//! cleanly in the Rust tests and fail in the field.
//!
//! So this test hands `tests/smoke/conform_smoke.c` to `cc`, links it against
//! the `cdylib` cargo just built, and runs it against this repository's own
//! registry and a fixture from `conform-odcs`.
//!
//! # Why this test is behind a feature
//!
//! It needs a C toolchain, and a plain `cargo test --workspace` for this
//! repository must not. `cargo test -p conform-ffi --features c-smoke` runs
//! it; so does CI's `cargo test --workspace --all-features`.
//!
//! # What it does not do
//!
//! It is not a sanitiser run. Address-sanitising this means rebuilding the
//! Rust side with different flags and relinking, which is a script's job and
//! not a `#[test]`'s; see `tools/sanitise/README.md` for what runs where, and
//! for what could and could not be run on the machine this was written on.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace root — the directory `specs.toml` lives in.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-ffi sits two levels below the workspace root")
        .to_path_buf()
}

/// The directory cargo put this test binary's artefacts in.
///
/// Derived from the running test binary rather than assumed to be
/// `target/debug`, so a `CARGO_TARGET_DIR`, a custom profile or a
/// cross-compiled target directory all work without this file knowing about
/// them.
fn artefact_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("a test binary knows where it is");
    exe.parent()
        .and_then(Path::parent)
        .expect("a test binary lives in <artefacts>/deps/")
        .to_path_buf()
}

/// The shared library a C program links against.
fn shared_library() -> PathBuf {
    let dir = artefact_dir();
    let candidates = [
        "libconform_ffi.dylib",
        "libconform_ffi.so",
        "conform_ffi.dll",
    ];
    for name in candidates {
        let path = dir.join(name);
        if path.exists() {
            return path;
        }
    }
    panic!(
        "no `conform-ffi` shared library in {}. The crate declares `crate-type = [\"cdylib\", \
         \"staticlib\", \"rlib\"]`, so `cargo test -p conform-ffi --features c-smoke` should \
         have built one; if it did not, `cargo build -p conform-ffi` first.",
        dir.display()
    );
}

/// Which C compiler to use. `CC` if the environment names one, else `cc`.
fn compiler() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_owned())
}

#[test]
fn a_c_program_links_the_library_and_validates_a_document() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let library = shared_library();
    let library_dir = library.parent().expect("a file has a directory").to_owned();

    let out_dir = artefact_dir().join("conform-ffi-smoke");
    std::fs::create_dir_all(&out_dir).expect("the artefact directory is writable");
    let program = out_dir.join("conform_smoke");

    let compile = Command::new(compiler())
        .arg("-std=c11")
        .args(["-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(crate_dir.join("include"))
        .arg(crate_dir.join("tests/smoke/conform_smoke.c"))
        .arg("-L")
        .arg(&library_dir)
        .arg("-lconform_ffi")
        // So the program finds the library at run time without anybody having
        // to set `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH` — which on macOS is
        // stripped from a child process by System Integrity Protection, and so
        // would not survive being set here anyway.
        .arg(format!("-Wl,-rpath,{}", library_dir.display()))
        .arg("-o")
        .arg(&program)
        .output()
        .expect("a C compiler should be on PATH; this test is gated behind `--features c-smoke`");

    assert!(
        compile.status.success(),
        "the C program did not compile against the generated header.\n--- stdout\n{}\n--- \
         stderr\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    // A deliberately faulty contract, so that "a report came back" and "a
    // report with findings in it came back" are different assertions and the
    // C program can make the second one.
    let document =
        workspace_root().join("crates/conform-odcs/tests/fixtures/faulty-many-faults.yaml");
    assert!(document.exists(), "{} is missing", document.display());

    let run = Command::new(&program)
        .arg(workspace_root().join("specs.toml"))
        .arg(&document)
        .output()
        .expect("the compiled program should run");

    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);

    assert!(
        run.status.success(),
        "the C program reported a failure (exit {:?}).\n--- stdout\n{stdout}\n--- stderr\n{stderr}",
        run.status.code(),
    );
    assert!(
        stdout.contains("all checks passed"),
        "the C program exited 0 without getting to the end.\n--- stdout\n{stdout}"
    );

    // The panic the self-test raises prints a Rust panic message on its way
    // past. Asserting it is there is asserting that the panic really happened
    // — a `conform_self_test_panic` that quietly returned the right code
    // without panicking would prove nothing.
    assert!(
        stderr.contains("deliberate panic from conform_self_test_panic"),
        "the self-test did not actually panic.\n--- stderr\n{stderr}"
    );
}
