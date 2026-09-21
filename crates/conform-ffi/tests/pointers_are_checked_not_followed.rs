//! Every pointer that crosses is checked before it is followed.
//!
//! # What can be checked, and what cannot
//!
//! A C caller can hand this library anything. Three kinds of "anything" are
//! detectable and are turned into status codes here:
//!
//! - **null**, for every pointer the call cannot do without;
//! - **misaligned**, for a handle, before anything reads through it;
//! - **not a handle**, by a guard word at offset zero that real handles carry
//!   and arbitrary memory almost never does.
//!
//! One kind is not detectable by anybody: a pointer into memory the process
//! has not mapped. Reading eight bytes there faults, and there is no portable
//! way to test a pointer before following it. The library documents that and
//! this file does not pretend otherwise.
//!
//! # Why there is no "use the handle after freeing it" test
//!
//! `conform_validator_free` poisons the guard word before releasing the
//! allocation, precisely so that a stale handle is *likely* to be refused
//! rather than followed. Writing a test that passes a freed pointer back in
//! would mean the test itself performing a use-after-free read — genuinely
//! undefined behaviour, which a sanitiser would rightly flag, and which would
//! make this file the least trustworthy thing in the crate.
//!
//! So the poison branch is exercised the honest way instead: by a buffer this
//! test owns, holding the poison word. That proves the *branch*, which is all
//! a test can prove; what the poison is worth in the field is a probability,
//! and the module documentation says so rather than this file implying
//! otherwise.

#![allow(
    unsafe_code,
    reason = "calling a C ABI from Rust is unsafe by construction; a test of \
              that ABI that could not write `unsafe` would be a test of \
              something else"
)]

mod support;

use std::ffi::{CString, c_char};

use conform_ffi::{ConformStatus, ConformValidator, abi};

use support::{c, in_workspace, last_error, release, specs_toml, validate, validator};

/// A chunk of memory this test owns, aligned and large enough that reading a
/// guard word out of it is reading our own initialised bytes rather than
/// anybody else's.
#[repr(align(16))]
struct Buffer([u8; 64]);

impl Buffer {
    fn zeroed() -> Self {
        Self([0; 64])
    }

    fn holding(word: [u8; 8]) -> Self {
        let mut buffer = Self([0; 64]);
        buffer.0[..8].copy_from_slice(&word);
        buffer
    }

    fn as_handle(&mut self) -> *mut ConformValidator {
        std::ptr::from_mut(self).cast::<ConformValidator>()
    }
}

#[test]
fn a_null_spec_id_is_refused_rather_than_read() {
    let registry = c(&specs_toml().display().to_string());
    // SAFETY: a null first argument, which the API documents as accepted and
    // refused, and a live C string second.
    let handle = unsafe { abi::conform_validator_new(std::ptr::null(), registry.as_ptr()) };

    assert!(handle.is_null());
    assert!(last_error().contains("spec id"), "{}", last_error());
}

#[test]
fn a_null_registry_path_is_refused_rather_than_read() {
    let spec = c("odcs");
    // SAFETY: a live C string first, a null second.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), std::ptr::null()) };

    assert!(handle.is_null());
    assert!(last_error().contains("registry path"), "{}", last_error());
}

#[test]
fn a_spec_id_that_is_not_utf8_is_refused() {
    // A C string is a byte string; nothing stops a caller passing bytes that
    // are not UTF-8, and `CStr::to_str` is where that stops being our problem.
    let spec = CString::new([0xff_u8, 0xfe]).expect("no interior NUL");
    let registry = c(&specs_toml().display().to_string());
    // SAFETY: two live, NUL-terminated byte strings.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), registry.as_ptr()) };

    assert!(handle.is_null());
    assert!(last_error().contains("not valid UTF-8"), "{}", last_error());
}

#[test]
fn a_null_handle_is_refused_rather_than_dereferenced() {
    let id = c("orders.yaml");
    let document = b"version: 1.0.0\n";
    let mut json: *mut c_char = std::ptr::null_mut();

    // SAFETY: a null handle, which the API documents as refused, plus live
    // arguments and a writable out-parameter.
    let status = unsafe {
        abi::conform_validate(
            std::ptr::null_mut(),
            id.as_ptr(),
            document.as_ptr(),
            document.len(),
            &raw mut json,
            std::ptr::null_mut(),
        )
    };

    assert_eq!(status, ConformStatus::NullPointer);
    assert!(
        json.is_null(),
        "a failed call left a pointer to free behind"
    );
    assert!(last_error().contains("null"), "{}", last_error());
}

#[test]
fn memory_that_is_not_a_handle_is_refused_rather_than_dereferenced() {
    let mut buffer = Buffer::zeroed();
    let id = c("orders.yaml");
    let document = b"version: 1.0.0\n";
    let mut json: *mut c_char = std::ptr::null_mut();

    // SAFETY: the "handle" points at 64 bytes this test owns and initialised,
    // so reading a guard word out of it is defined; what it is *not* is a
    // validator, which is the thing being tested.
    let status = unsafe {
        abi::conform_validate(
            buffer.as_handle(),
            id.as_ptr(),
            document.as_ptr(),
            document.len(),
            &raw mut json,
            std::ptr::null_mut(),
        )
    };

    assert_eq!(status, ConformStatus::InvalidHandle);
    assert!(json.is_null());
    assert!(last_error().contains("guard word"), "{}", last_error());
}

#[test]
fn a_poisoned_handle_is_reported_as_already_freed() {
    // The bytes `conform_validator_free` writes over the guard word before
    // releasing the allocation. Written as bytes rather than as an integer
    // because that is what the library compares — the guard is a *byte
    // pattern* at offset zero, and going through a `u64` here would make this
    // test agree with the library only on a little-endian host.
    //
    // See this file's opening note for why the poison branch is exercised
    // through owned memory rather than through a real freed handle.
    const POISON: [u8; 8] = *b"DEADconf";

    let mut buffer = Buffer::holding(POISON);
    let id = c("orders.yaml");
    let mut json: *mut c_char = std::ptr::null_mut();

    // SAFETY: as above — 64 initialised bytes this test owns.
    let status = unsafe {
        abi::conform_validate(
            buffer.as_handle(),
            id.as_ptr(),
            std::ptr::null(),
            0,
            &raw mut json,
            std::ptr::null_mut(),
        )
    };

    assert_eq!(status, ConformStatus::InvalidHandle);
    assert!(
        last_error().contains("already been freed"),
        "the poison was detected but not explained: {}",
        last_error()
    );
}

#[test]
fn a_misaligned_handle_is_refused_before_anything_reads_through_it() {
    let mut buffer = Buffer::zeroed();
    // One byte into an aligned buffer, which cannot be a `ConformValidator`
    // and must be rejected *before* the guard-word read rather than by it.
    let misaligned = std::ptr::from_mut(&mut buffer)
        .cast::<u8>()
        .wrapping_add(1)
        .cast::<ConformValidator>();

    // SAFETY: still inside a 64-byte buffer this test owns; the call is
    // expected to refuse it on alignment and read nothing.
    unsafe { abi::conform_validator_free(misaligned) };

    assert!(last_error().contains("misaligned"), "{}", last_error());
}

#[test]
fn a_null_document_id_is_refused() {
    let handle = validator("odcs");
    let document = b"version: 1.0.0\n";
    let mut json: *mut c_char = std::ptr::null_mut();

    // SAFETY: a live handle, a null id, a readable slice, a writable
    // out-parameter.
    let status = unsafe {
        abi::conform_validate(
            handle,
            std::ptr::null(),
            document.as_ptr(),
            document.len(),
            &raw mut json,
            std::ptr::null_mut(),
        )
    };
    release(handle);

    assert_eq!(status, ConformStatus::NullPointer);
    assert!(json.is_null());
    assert!(last_error().contains("document id"), "{}", last_error());
}

#[test]
fn a_null_document_with_a_non_zero_length_is_refused() {
    let handle = validator("odcs");
    let id = c("orders.yaml");
    let mut json: *mut c_char = std::ptr::null_mut();

    // SAFETY: a live handle and id; the null/length mismatch is the thing
    // under test and is refused before any slice is formed.
    let status = unsafe {
        abi::conform_validate(
            handle,
            id.as_ptr(),
            std::ptr::null(),
            32,
            &raw mut json,
            std::ptr::null_mut(),
        )
    };
    release(handle);

    assert_eq!(status, ConformStatus::NullPointer);
    assert!(json.is_null());
}

#[test]
fn a_null_document_with_a_zero_length_is_an_empty_document() {
    // The one case where null is *not* an error: a caller with nothing to
    // validate has no buffer to point at, and every C library in the world
    // accepts `(NULL, 0)`. An empty ODCS contract is a finding, not a refusal.
    let handle = validator("odcs");
    let id = c("empty.yaml");
    let mut json: *mut c_char = std::ptr::null_mut();
    let mut length = usize::MAX;

    // SAFETY: a live handle and id, and the documented `(null, 0)` pair.
    let status = unsafe {
        abi::conform_validate(
            handle,
            id.as_ptr(),
            std::ptr::null(),
            0,
            &raw mut json,
            &raw mut length,
        )
    };

    assert_eq!(status, ConformStatus::Ok, "{}", last_error());
    assert!(!json.is_null());
    assert_ne!(length, usize::MAX);

    // SAFETY: freed exactly once, with the pointer the call handed back.
    unsafe { abi::conform_string_free(json) };
    release(handle);
}

#[test]
fn a_null_output_parameter_is_refused_rather_than_written_through() {
    let handle = validator("odcs");
    let id = c("orders.yaml");
    let document = b"version: 1.0.0\n";

    // SAFETY: a live handle, a live id, a readable slice, and a null
    // out-parameter, which is the thing under test.
    let status = unsafe {
        abi::conform_validate(
            handle,
            id.as_ptr(),
            document.as_ptr(),
            document.len(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    release(handle);

    assert_eq!(status, ConformStatus::NullPointer);
    assert!(last_error().contains("out_json"), "{}", last_error());
}

#[test]
fn a_document_that_is_not_utf8_is_refused() {
    let handle = validator("odcs");
    // A lone continuation byte: not UTF-8 by any reading.
    let (status, json) = validate(handle, "orders.yaml", &[0x80, 0x80, 0x80]);
    release(handle);

    assert_eq!(status, ConformStatus::InvalidUtf8);
    assert!(json.is_none());
    assert!(last_error().contains("UTF-8"), "{}", last_error());
}

#[test]
fn freeing_null_is_allowed_and_does_nothing() {
    // So that a C caller can free unconditionally in its cleanup path, which
    // is the only cleanup path anybody writes correctly.
    // SAFETY: null is documented as accepted by both.
    unsafe {
        abi::conform_string_free(std::ptr::null_mut());
        abi::conform_validator_free(std::ptr::null_mut());
    }
}

#[test]
fn an_unknown_spec_id_is_refused_before_the_registry_is_opened() {
    let spec = c("no-such-standard");
    // A path that does not exist: if the registry were opened first, *that*
    // would be the complaint, and the caller would be sent to fix the wrong
    // thing.
    let registry = c(&in_workspace("no/such/specs.toml").display().to_string());
    // SAFETY: two live, NUL-terminated C strings.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), registry.as_ptr()) };

    assert!(handle.is_null());
    let message = last_error();
    assert!(message.contains("no-such-standard"), "{message}");
    assert!(message.contains("odcs"), "{message}");
    assert!(
        !message.contains("no/such/specs.toml"),
        "the id was rejected, so the path should not be mentioned: {message}"
    );
}

#[test]
fn the_bundle_standard_is_refused_with_the_reason_it_cannot_work() {
    // `okf` is in the registry and the workspace has an adapter for it, so a
    // reader will reasonably expect this to work. It cannot: an OKF bundle is
    // a directory, and this boundary takes a document's bytes. The refusal has
    // to say that rather than read like a typo.
    let spec = c("okf");
    let registry = c(&specs_toml().display().to_string());
    // SAFETY: two live, NUL-terminated C strings.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), registry.as_ptr()) };

    assert!(handle.is_null());
    let message = last_error();
    assert!(message.contains("bundle"), "{message}");
    assert!(message.contains("conform-okf"), "{message}");
}

#[test]
fn a_registry_that_is_not_there_is_reported_as_such() {
    let spec = c("odcs");
    let registry = c(&in_workspace("no/such/specs.toml").display().to_string());
    // SAFETY: two live, NUL-terminated C strings.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), registry.as_ptr()) };

    assert!(handle.is_null());
    assert!(
        last_error().contains("specs.toml"),
        "the refusal does not say which file it could not read: {}",
        last_error()
    );
}
