//! A panic inside this library must never reach the C frame that called in.
//!
//! # Why this is the most important test in the crate
//!
//! Unwinding across an `extern "C"` boundary is undefined behaviour. Not
//! "discouraged", not "implementation-defined": undefined, and in practice it
//! means the host process dies — or worse, does not, and carries on with a
//! stack the C compiler's exception tables never described. A conformance
//! library that can do that is a library that can take down a build server
//! over an off-by-one in a diagnostic message.
//!
//! Every entry point in [`conform_ffi::abi`] therefore wraps its whole body in
//! [`std::panic::catch_unwind`]. This file proves that the net is real by
//! forcing a panic through it.
//!
//! # How this test proves anything
//!
//! `conform_self_test_panic` panics on purpose. If the net were missing, the
//! unwind would leave the `extern "C"` frame and this test binary would abort
//! — a test that *fails by dying*, which is the only honest way to test this.
//! So the assertions below are the second half of the proof; the first half is
//! that the process is still here to run them.
//!
//! A Rust panic message is printed to stderr on the way past. That is
//! deliberate and is documented on the function: suppressing it would mean a
//! library installing a process-global panic hook, which is not a library's to
//! install.

#![allow(
    unsafe_code,
    reason = "calling a C ABI from Rust is unsafe by construction; a test of \
              that ABI that could not write `unsafe` would be a test of \
              something else"
)]

mod support;

use conform_ffi::{ConformStatus, abi};

use support::{last_error, release, validate, validator};

#[test]
fn a_deliberate_panic_becomes_a_status_code_rather_than_an_abort() {
    let status = abi::conform_self_test_panic();

    // Reaching this line at all is half the assertion.
    assert_eq!(
        status,
        ConformStatus::Panic,
        "the boundary caught something, but reported it as `{status}`"
    );
}

#[test]
fn the_caught_panic_explains_itself() {
    let _ = abi::conform_self_test_panic();
    let message = last_error();

    // The payload, so a maintainer can find the panic site, and the entry
    // point, so they know which call it came out of. A status code with no
    // string behind it would tell a C caller that something went wrong and
    // nothing else.
    assert!(
        message.contains("deliberate panic from conform_self_test_panic"),
        "the panic payload was lost: {message}"
    );
    assert!(
        message.contains("conform_self_test_panic"),
        "the entry point was not named: {message}"
    );
}

#[test]
fn the_library_still_works_after_catching_one() {
    // A caught panic must leave the library usable. If `catch_unwind` were
    // papering over a half-updated global — a poisoned lock, a leaked borrow
    // on the error slot — the call after it would be the one that failed, and
    // it would fail a long way from the cause.
    assert_eq!(abi::conform_self_test_panic(), ConformStatus::Panic);

    let handle = validator("odcs");
    let (status, json) = validate(handle, "orders.yaml", b"version: 1.0.0\n");
    release(handle);

    assert_eq!(status, ConformStatus::Ok, "{}", last_error());
    assert!(json.is_some_and(|json| json.contains("\"schema_version\"")));
}

#[test]
fn a_successful_call_clears_the_error_left_by_a_caught_panic() {
    let _ = abi::conform_self_test_panic();
    assert!(last_error().contains("panicked"));

    let handle = validator("odcs");
    let (status, _) = validate(handle, "orders.yaml", b"version: 1.0.0\n");
    release(handle);

    assert_eq!(status, ConformStatus::Ok);
    assert_eq!(
        conform_ffi::error::last_error(),
        None,
        "a stale panic message survived a successful call, so a C caller \
         reading the slot after a success would be told about a failure that \
         is no longer happening"
    );
}
