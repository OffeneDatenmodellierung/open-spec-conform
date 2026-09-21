//! `include/conform.h` is what the library actually exports, or this fails.
//!
//! # Why a committed header needs a test at all
//!
//! A generated file that is committed is a file that rots. The generator runs
//! on somebody's laptop, the output is pasted into the tree, and six months
//! later a function has grown an argument that the header still describes the
//! old way — at which point every C caller compiles cleanly and passes the
//! wrong number of arguments. A header is not documentation; it is the thing
//! the compiler believes, and a stale one is worse than none, because none at
//! least fails loudly.
//!
//! So the header is regenerated here, from the same generator and the same
//! configuration that produced the committed copy, and compared byte for byte.
//!
//! # Why the header is committed rather than generated at build time
//!
//! Because a C program that links this library should not have to run
//! `cbindgen` — or have a Rust toolchain at all — to find out what it is
//! linking. Committing the header also makes an ABI change visible in a diff,
//! which is the only place a human will realistically catch one.
//!
//! # Why the generator is a separate crate outside the workspace
//!
//! `cbindgen` is MPL-2.0. `deny.toml`'s allow-list is permissive licences
//! only, and `cargo deny --all-features` resolves the whole workspace graph
//! including optional features, so taking `cbindgen` as a dependency of this
//! crate fails the licence gate:
//!
//! ```text
//! error[rejected]: failed to satisfy license requirements
//!    41 │ license = "MPL-2.0"
//!       │            rejected: license is not explicitly allowed
//!    ├ cbindgen v0.29.4
//!      └── conform-ffi v0.1.0
//! ```
//!
//! That finding is correct and an exception for it would blur the distinction
//! the gate exists to keep sharp — between what this repository *ships* and
//! what it *builds with*. The plan says so in as many words (§5.3, risk R4):
//! `cbindgen` must not appear in the harness's tree. So the generator lives in
//! `tools/headergen`, outside the workspace, exactly as `tools/oracle` does
//! and for the same reason, and this test runs it.
//!
//! # Updating the header
//!
//! ```sh
//! cargo run --manifest-path tools/headergen/Cargo.toml
//! ```
//!
//! Then read the diff. That is the ABI change, and it is the diff a reviewer
//! should be looking at.
//!
//! # Why this test is behind a feature
//!
//! It shells out to `cargo`, which will compile `cbindgen` the first time and
//! wants a network to do it. A plain `cargo test --workspace` must not need
//! either. CI runs `cargo test --workspace --all-features`, which includes
//! this.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace root — the directory `tools/` and `specs.toml` live in.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-ffi sits two levels below the workspace root")
        .to_path_buf()
}

/// Where the committed header lives.
fn header_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("include")
        .join("conform.h")
}

/// Regenerate the header into a scratch file and read it back.
fn generate() -> String {
    let root = workspace_root();
    let manifest = root.join("tools/headergen/Cargo.toml");
    assert!(
        manifest.exists(),
        "{} is missing, so the header cannot be regenerated",
        manifest.display()
    );

    // Beside the test binary rather than in the source tree, so a failing run
    // leaves nothing behind for the next one to compare against by accident.
    let destination =
        std::env::temp_dir().join(format!("conform-ffi-header-{}.h", std::process::id()));

    let run = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--")
        .arg(&destination)
        .current_dir(&root)
        .output()
        .expect("cargo should be runnable from a cargo test");

    assert!(
        run.status.success(),
        "the header generator failed.\n--- stdout\n{}\n--- stderr\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );

    let generated = std::fs::read_to_string(&destination)
        .unwrap_or_else(|error| panic!("{} was not written: {error}", destination.display()));
    let _ = std::fs::remove_file(&destination);
    generated
}

/// The first line at which two texts differ, so the failure points at
/// something rather than printing two files and leaving the reader to diff
/// them.
fn first_difference(committed: &str, generated: &str) -> String {
    for (index, (left, right)) in committed.lines().zip(generated.lines()).enumerate() {
        if left != right {
            return format!(
                "first difference at line {}:\n  committed: {left}\n  generated: {right}",
                index + 1
            );
        }
    }
    format!(
        "the files agree line for line as far as the shorter one goes; the committed copy has {} \
         lines and the generated one has {}",
        committed.lines().count(),
        generated.lines().count()
    )
}

#[test]
fn the_committed_header_is_what_the_generator_produces_today() {
    let generated = generate();
    let path = header_path();

    let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} could not be read ({error}). Generate it with:\n    cargo run --manifest-path \
             tools/headergen/Cargo.toml",
            path.display()
        )
    });

    assert_eq!(
        committed,
        generated,
        "{} no longer describes this library.\n{}\n\nRegenerate with:\n    cargo run \
         --manifest-path tools/headergen/Cargo.toml\n\nand read the diff: it is the ABI change.",
        path.display(),
        first_difference(&committed, &generated)
    );
}

#[test]
fn the_header_declares_every_entry_point_the_library_exports() {
    // A belt to the braces above. The comparison catches drift between the
    // header and the generator's reading of the source; this catches the case
    // where *both* are wrong because a function stopped being exported — the
    // header and the generator would agree perfectly, and the C caller would
    // get a link error with no explanation.
    let committed = std::fs::read_to_string(header_path()).expect("the committed header");

    for entry_point in [
        "conform_version",
        "conform_validator_new",
        "conform_validate",
        "conform_last_error",
        "conform_string_free",
        "conform_validator_free",
        "conform_self_test_panic",
    ] {
        assert!(
            committed.contains(entry_point),
            "{entry_point} is missing from the header"
        );
    }

    // The status codes a caller has to be able to name.
    for code in [
        "CONFORM_STATUS_OK",
        "CONFORM_STATUS_NULL_POINTER",
        "CONFORM_STATUS_INVALID_HANDLE",
        "CONFORM_STATUS_INVALID_UTF8",
        "CONFORM_STATUS_UNKNOWN_SPEC",
        "CONFORM_STATUS_REGISTRY",
        "CONFORM_STATUS_SERIALISATION",
        "CONFORM_STATUS_PANIC",
    ] {
        assert!(
            committed.contains(code),
            "{code} is missing from the header"
        );
    }

    // The two compile-time constants: how a binding refuses a library it was
    // not written against, and how it refuses a report it cannot parse.
    assert!(committed.contains("CONFORM_ABI_VERSION"));
    assert!(committed.contains("CONFORM_SCHEMA_VERSION"));

    // And the handle stays opaque. If this line ever fails it is because the
    // header has started publishing the guard word and a pointer into this
    // crate's internals, which would make the layout part of the ABI.
    assert!(
        committed.contains("typedef struct ConformValidator ConformValidator;"),
        "the handle is no longer opaque in the header"
    );
}
