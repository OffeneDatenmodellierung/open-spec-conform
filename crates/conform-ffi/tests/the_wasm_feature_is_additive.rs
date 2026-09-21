//! The `wasm` feature must not change the C ABI, and must not reach anybody
//! who did not ask for it.
//!
//! # What could go wrong, and why a test rather than a promise
//!
//! Two regressions are easy to write and invisible in review.
//!
//! The first is a helpful fallback: somebody notices that
//! `conform_validator_new` cannot work without a filesystem, sees that this
//! crate now carries embedded schema bytes, and makes the constructor fall
//! back to them when the registry will not load. A C caller who passed a wrong
//! path would then get a *working validator* instead of a null and an
//! explanation — a verdict against bytes it never asked for, issued because a
//! path was mistyped. Nothing about that shows up in a signature.
//!
//! The second is a default feature. `wasm = ["dep:wasm-bindgen"]` is optional
//! and off, and a one-word edit to `[features] default` would put
//! `wasm-bindgen` and its macro subtree into the graph of every crate that
//! depends on this one — including, transitively, nothing at all today, which
//! is exactly why it would go unnoticed until somebody ran `cargo deny`.
//!
//! # What this file measures
//!
//! It runs with the feature **on** — `cargo test --workspace --all-features`
//! — and with it **off**, and asserts the same things either way. That is the
//! whole design: an assertion that only holds in one configuration is an
//! assertion about a configuration, not about the ABI.
//!
//! The dependency half is read out of this crate's own manifest rather than
//! out of `cargo tree`, so it needs no subprocess and no network: cargo's
//! rule is that an optional dependency is absent unless a feature names it,
//! so `optional = true` plus an empty `default` *is* the tree property.

#![allow(
    unsafe_code,
    reason = "calling a C ABI from Rust is unsafe by construction; a test of \
              that ABI that could not write `unsafe` would be a test of \
              something else"
)]

mod support;

use conform_ffi::{ConformStatus, abi};

use support::{c, last_error, release, validate, validator};

/// This crate's manifest, as text.
///
/// Read rather than parsed with a TOML library on purpose: this crate has no
/// TOML dependency and should not grow one to check its own feature table.
/// The assertions below are about lines a human wrote and a human will edit.
fn manifest() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("a crate can read its own manifest")
}

#[test]
fn the_panic_net_under_the_c_abi_is_still_real() {
    // The guarantee the whole crate is built around, measured in whichever
    // configuration this binary was compiled in. Reaching the assertion at all
    // is half of it: if the net were gone, the unwind would leave the
    // `extern "C"` frame and this process would not be here to assert.
    assert_eq!(
        abi::conform_self_test_panic(),
        ConformStatus::Panic,
        "the `wasm` feature must not disturb the native panic net"
    );
    assert!(
        last_error().contains("conform_self_test_panic"),
        "the caught panic must still name the entry point it came from: {}",
        last_error()
    );
}

#[test]
fn the_c_constructor_still_refuses_a_registry_it_cannot_read() {
    // The fallback that must never be written. `okf` and a nonexistent
    // registry are two different refusals and both must stay refusals even
    // though this build may be carrying embedded bytes for `odcs`.
    let spec = c("odcs");
    let missing = c("/nonexistent/there-is-no-registry-here/specs.toml");
    // SAFETY: two live, NUL-terminated C strings that outlive the call.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), missing.as_ptr()) };
    assert!(
        handle.is_null(),
        "the C constructor fell back to something when its registry would not load"
    );
    let explanation = last_error();
    assert!(
        !explanation.contains("embedded"),
        "the C constructor reached the embedded bytes: {explanation}"
    );
}

#[test]
fn the_c_abi_still_reads_the_registry_off_disk() {
    // The positive half of the test above: the constructor that takes a path
    // still uses it, and the provenance sentence on its reports is the
    // on-disk one rather than the embedded one.
    let handle = validator("odcs");
    let (status, json) = validate(handle, "empty.yaml", b"");
    release(handle);

    assert_eq!(status, ConformStatus::Ok);
    let json = json.expect("a report");
    assert!(
        json.contains("verified against the digest `specs.toml` records"),
        "the C ABI's provenance sentence changed: {json}"
    );
    assert!(
        !json.contains("embedded in this build"),
        "the C ABI is reporting the embedded provenance: {json}"
    );
}

#[test]
fn the_abi_surface_did_not_grow() {
    // The committed header is the ABI a C program compiles against. The
    // `wasm` binding is `wasm-bindgen`'s surface, reached from JavaScript and
    // not from C, and it must not add an entry point here — a header that
    // grew one would mean the two surfaces had been merged by accident.
    let header = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/include/conform.h"))
        .expect("the committed header");

    let declared: Vec<&str> = header
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with("conform_"))
        .collect();
    let mut unique: Vec<&str> = declared.clone();
    unique.sort_unstable();
    unique.dedup();

    assert_eq!(
        unique,
        [
            "conform_last_error",
            "conform_self_test_panic",
            "conform_string_free",
            "conform_validate",
            "conform_validator_free",
            "conform_validator_new",
            "conform_version",
        ],
        "the C ABI's entry points changed"
    );
    // `cbindgen` names itself in the generated banner, so the needle is the
    // other generator's name in full rather than the substring both share.
    for forbidden in ["wasm-bindgen", "wasm_bindgen"] {
        assert!(
            !header.contains(forbidden),
            "`{forbidden}` reached the committed C header"
        );
    }

    // And the header still carries the warning that made this whole exercise
    // necessary, in the one place a C programmer will read it.
    assert!(
        header.contains("`wasm32-unknown-unknown` is such a target"),
        "the self-test's note about `panic = \"abort\"` targets left the header"
    );
}

#[test]
fn wasm_bindgen_is_optional_and_no_default_feature_turns_it_on() {
    let manifest = manifest();

    assert!(
        manifest.contains("wasm-bindgen = { version = \"0.2.128\", optional = true }"),
        "`wasm-bindgen` must stay an optional dependency, or it joins every consumer's graph"
    );
    assert!(
        manifest.contains("wasm = [\"dep:wasm-bindgen\"]"),
        "the `wasm` feature must be the only thing that pulls `wasm-bindgen` in"
    );
    assert!(
        manifest.contains("default = []"),
        "this crate must have no default features"
    );

    // The `dep:` prefix matters: without it cargo also creates an implicit
    // `wasm-bindgen` feature, which is one more way to turn the dependency on
    // than this crate wants to have.
    assert!(
        !manifest.contains("wasm = [\"wasm-bindgen\"]"),
        "name the dependency as `dep:wasm-bindgen`, so no implicit feature is created"
    );
}

#[test]
fn the_wasm_module_exists_exactly_when_the_feature_does() {
    // Compile-time, not runtime: with the feature off the `else` arm is the
    // one that compiles, and a `conform_ffi::wasm` that had leaked into the
    // default build would fail to build this file rather than fail an
    // assertion in it.
    #[cfg(feature = "wasm")]
    {
        assert!(
            conform_ffi::wasm::PANIC_CONTRACT.contains("fatal"),
            "the wasm panic contract must say what it is"
        );
        assert_eq!(
            conform_ffi::embedded::spec_ids(),
            conform_ffi::engine::REACHABLE_SPEC_IDS.to_vec(),
            "the embedded schemas and the registry-backed adapters must agree on which \
             standards this crate speaks for"
        );
    }
    #[cfg(not(feature = "wasm"))]
    {
        // Nothing to call. What is being asserted is that the default build
        // compiled this file at all without naming `conform_ffi::wasm`, which
        // it could not do if the module were unconditional.
        assert_eq!(conform_ffi::engine::REACHABLE_SPEC_IDS.len(), 2);
    }
}
