//! `include/conform.h` is what the library actually exports, or this fails.
//!
//! # Why a committed header needs a test at all
//!
//! A generated file that is committed is a file that rots. The generator runs
//! on somebody's laptop, the output is pasted into the tree, and six months
//! later a function has grown an argument that the header still describes the
//! old way — at which point every C caller compiles cleanly and passes the
//! wrong number of arguments. A header is not documentation; it is the thing
//! the compiler believes, and a stale one is worse than none because none at
//! least fails loudly.
//!
//! So the header is regenerated here, from the same configuration that
//! produced the committed copy, and compared byte for byte.
//!
//! # Why the header is committed rather than generated at build time
//!
//! Because a C program that links this library should not have to run
//! `cbindgen` — or have a Rust toolchain at all — to find out what it is
//! linking. Committing the header makes the ABI reviewable in a diff, which is
//! the only place an ABI change can realistically be caught by a human.
//!
//! # Updating it
//!
//! ```sh
//! cargo test -p conform-ffi --features header-check          # shows the drift
//! CONFORM_UPDATE_HEADER=1 cargo test -p conform-ffi --features header-check
//! ```
//!
//! The second form writes the file and then fails anyway, so that a run which
//! *changed the committed ABI* can never be mistaken for a run that found
//! nothing to change.
//!
//! # Why this test is behind a feature
//!
//! `cbindgen` is a compiler front end with a `syn` tree behind it. Building it
//! for every `cargo test --workspace` in this repository, for the benefit of
//! one assertion in one crate, is not a trade worth making. CI runs
//! `cargo test --workspace --all-features`, which includes it.

use std::path::{Path, PathBuf};

/// Where the committed header lives.
fn header_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("include")
        .join("conform.h")
}

/// Regenerate the header from `src/`, using this crate's `cbindgen.toml`.
fn generate() -> String {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml"))
        .expect("crates/conform-ffi/cbindgen.toml should be readable and valid");

    let bindings = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("the crate's public C ABI should be describable as a header");

    let mut rendered = Vec::new();
    bindings.write(&mut rendered);
    String::from_utf8(rendered).expect("cbindgen emits UTF-8")
}

/// The first line at which two texts differ, for a message that points at
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
        "the files agree for their first {} lines; the committed copy has {} and the generated \
         one has {}",
        committed.lines().count().min(generated.lines().count()),
        committed.lines().count(),
        generated.lines().count()
    )
}

#[test]
fn the_committed_header_is_what_cbindgen_produces_today() {
    let generated = generate();
    let path = header_path();

    if std::env::var_os("CONFORM_UPDATE_HEADER").is_some() {
        std::fs::write(&path, &generated).expect("the header should be writable");
        panic!(
            "CONFORM_UPDATE_HEADER was set, so {} has been rewritten. This test fails on \
             purpose: the ABI just changed, and a run that changes an ABI must not look like a \
             run that found nothing to change. Review the diff, then run without the variable.",
            path.display()
        );
    }

    let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} could not be read ({error}). Generate it with:\n    CONFORM_UPDATE_HEADER=1 \
             cargo test -p conform-ffi --features header-check",
            path.display()
        )
    });

    assert_eq!(
        committed,
        generated,
        "{} no longer describes this library.\n{}\n\nRegenerate with:\n    \
         CONFORM_UPDATE_HEADER=1 cargo test -p conform-ffi --features header-check",
        path.display(),
        first_difference(&committed, &generated)
    );
}

#[test]
fn the_header_declares_every_entry_point_the_library_exports() {
    // A belt to the braces above. The comparison catches drift between the
    // header and `cbindgen`'s reading of the source; this catches the case
    // where *both* are wrong because a function stopped being exported — the
    // header and the generator would agree, and the C caller would get a link
    // error with no explanation.
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

    // And the compile-time ABI version, which is how a binding refuses a
    // library it was not written against.
    assert!(committed.contains("CONFORM_ABI_VERSION"));
}
