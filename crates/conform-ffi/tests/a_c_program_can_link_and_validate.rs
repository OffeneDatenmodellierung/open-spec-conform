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
//! `libconform_ffi.a`, and runs it against this repository's own registry and
//! a fixture from `conform-odcs`.
//!
//! # It asks cargo for the library rather than assuming one is there
//!
//! This test used to look for the `cdylib` in `target/debug` and fail if it
//! was absent, with a panic message that helpfully suggested running
//! `cargo build` first. That message was the bug wearing a disguise: a test
//! that tells you how to arrange the world before running it is a test with an
//! ordering dependency it cannot satisfy.
//!
//! `cargo test` builds the lib target as an `rlib`, to link into the test
//! binaries, and **nothing else** — no `cdylib`, no `staticlib`. On a warm
//! tree one is lying around from an earlier `cargo build` and the test passes;
//! on a clean checkout — which is what CI always is — there is no such file,
//! and it does not. That is not a CI problem to be papered over with a build
//! step, because the same trap catches every contributor running the
//! documented command on a fresh clone.
//!
//! So the test asks cargo to produce the artefact, and asks cargo where it put
//! it, with one invocation:
//!
//! ```text
//! cargo rustc -p conform-ffi --lib --crate-type staticlib \
//!     --message-format=json --color=never -- --print native-static-libs
//! ```
//!
//! which answers both questions at once, on stdout, as two structured records
//! — the `compiler-artifact` giving the exact path and the `compiler-message`
//! giving the system libraries a Rust `staticlib` has to be linked against on
//! this platform. Neither is guessed. Hard-coding `libconform_ffi.a` and a
//! per-OS list of `-lpthread -ldl -lm` would be two more things that are right
//! only on the machine they were written on.
//!
//! `--message-format=json` rather than `json-render-diagnostics`, and that is
//! not a detail. The `-render-diagnostics` variant keeps *no* structured copy
//! of that note: it renders it to stderr and nothing else, which is what used
//! to force this test to scrape human-readable text — and on CI, where
//! `CARGO_TERM_COLOR: always` is set, the text it scraped had an ANSI reset in
//! it, which went straight to the linker. See
//! `tests/support/cargo_output.rs` for the full account and for the guard
//! that now stands behind the structured route regardless.
//!
//! Nesting cargo inside `cargo test` is safe here: by the time a test binary
//! runs, the outer invocation has finished building and released the build
//! directory, and every dependency the inner one needs is already compiled —
//! so it relinks one crate and returns.
//!
//! # Why the `staticlib` and not the `cdylib`
//!
//! Because `crate-type` declares three artefacts and something should link
//! each of them. The `rlib` is linked by every other test file here; the
//! `staticlib` is linked by this one, which closes the gap left when this test
//! took the `cdylib`. Embedding is also the harder of the two for a Rust
//! library to get right, because it is the one where the *caller* has to
//! supply the platform's own libraries — which is exactly what the
//! `native-static-libs` note above exists to tell it.
//!
//! It costs no coverage. `--crate-type staticlib` narrows this invocation to
//! the one artefact — `target/debug` holds `libconform_ffi.a` and nothing else
//! after it — but `tools/sanitise/run.sh` runs a plain `cargo build -p
//! conform-ffi`, which produces all three, and links the `cdylib` for its
//! valgrind arm. So each declared crate-type is linked by something, by a
//! different something, and neither has to trust the other to have run first.
//!
//! It also drops the `rpath` the dynamic version needed, so the compiled
//! program is a single file that runs with no environment at all.
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

mod support;

use support::cargo_output::{self, Linkage};

/// The workspace root — the directory `specs.toml` lives in.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-ffi sits two levels below the workspace root")
        .to_path_buf()
}

/// The cargo that is running this test, so the inner invocation is the same
/// toolchain as the outer one rather than whatever is first on `PATH`.
fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from)
}

/// Which C compiler to use. `CC` if the environment names one, else `cc`.
fn compiler() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_owned())
}

/// Build the library, and find out from cargo both where it is and what else
/// it needs.
fn build() -> Linkage {
    let run = Command::new(cargo())
        .args([
            "rustc",
            "-p",
            "conform-ffi",
            "--lib",
            "--crate-type",
            "staticlib",
            "--message-format=json",
            // Belt and braces with the structured route above. Nothing read
            // below comes from a rendered string any more, so colour cannot
            // reach a linker argument even if this were left on — but an
            // inherited `CARGO_TERM_COLOR=always` has already cost this
            // repository one red CI run, and turning it off at the call site
            // costs nothing. Both the flag and the variable, because the flag
            // covers cargo and the variable covers anything cargo spawns.
            "--color=never",
            "--",
            "--print",
            "native-static-libs",
        ])
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(workspace_root())
        .output()
        .expect("cargo should be runnable from a cargo test");

    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();

    // `--message-format=json` stops cargo rendering diagnostics to stderr, so
    // a build failure arrives as JSON. Rendering it here is the one use this
    // test makes of a `rendered` field: to show a human what went wrong, never
    // to take a value out of.
    assert!(
        run.status.success(),
        "cargo could not build the static library.\n--- diagnostics\n{}\n--- stderr\n{stderr}",
        cargo_output::rendered_diagnostics(&stdout),
    );

    cargo_output::linkage(&stdout).unwrap_or_else(|problem| {
        panic!("cargo's message stream could not be read: {problem}\n--- stdout\n{stdout}")
    })
}

#[test]
fn a_c_program_links_the_library_and_validates_a_document() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let linkage = build();
    assert!(
        linkage.archive.exists(),
        "cargo named {} and did not produce it",
        linkage.archive.display()
    );

    // Belt, braces, and a third thing. `cargo_output` refuses an unsafe token
    // before it gets here, but this is the last moment before the flags become
    // a command line, and the cost of saying so again is nothing.
    for flag in &linkage.native {
        assert!(
            cargo_output::is_safe_token(flag),
            "a library flag reached the command line unchecked: `{}`",
            cargo_output::show(flag)
        );
    }

    let out_dir = linkage
        .archive
        .parent()
        .expect("an artefact has a directory")
        .join("conform-ffi-smoke");
    std::fs::create_dir_all(&out_dir).expect("the artefact directory is writable");
    let program = out_dir.join("conform_smoke");

    let mut compile = Command::new(compiler());
    compile
        .arg("-std=c11")
        .args(["-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(crate_dir.join("include"))
        .arg(crate_dir.join("tests/smoke/conform_smoke.c"))
        // The archive before the system libraries it needs, which is the order
        // a traditional linker wants and the order rustc printed them in.
        .arg(&linkage.archive)
        .args(&linkage.native)
        .arg("-o")
        .arg(&program);

    let compiled = compile
        .output()
        .expect("a C compiler should be on PATH; this test is gated behind `--features c-smoke`");

    assert!(
        compiled.status.success(),
        "the C program did not compile and link against the generated header and the static \
         library.\ncommand: {compile:?}\n--- stdout\n{}\n--- stderr\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr),
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
