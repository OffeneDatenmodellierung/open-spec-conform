//! Driving the boundary the way a C caller does, from Rust.
//!
//! The crate is built as an `rlib` as well as a `cdylib`, so these tests call
//! the *real* `extern "C"` entry points — same `#[unsafe(no_mangle)]`
//! functions a C program links against, same `catch_unwind`, same pointer
//! checks. Nothing here is a Rust-shaped shadow of the API; the only
//! difference from C is that the caller happens to be written in Rust.

// Each integration test compiles this module separately, so a helper used by
// one of them is dead code in the others.
#![allow(dead_code)]
// Calling a C ABI from Rust is unsafe by construction. The crate's manifest
// denies `unsafe_code` so that the *library* has to opt out in writing, in one
// module; a test of that ABI has to opt out too, and for the same reason it is
// worth writing down rather than inheriting.
#![allow(
    unsafe_code,
    reason = "these helpers are a C caller written in Rust; every call below \
              is one of the library's documented pointer contracts"
)]

use std::ffi::{CStr, CString, c_char};
use std::path::{Path, PathBuf};

use conform_ffi::{ConformStatus, abi, error};

/// The workspace root — the directory `specs.toml` lives in.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-ffi should sit two levels below the workspace root")
        .to_path_buf()
}

/// The repository's real registry.
///
/// Passed explicitly rather than searched for, because this library does not
/// search — and because a test that depended on where `cargo` happened to be
/// invoked from would be a test that passes for the wrong reason.
pub fn specs_toml() -> PathBuf {
    workspace_root().join("specs.toml")
}

/// A path inside the workspace.
pub fn in_workspace(relative: &str) -> PathBuf {
    workspace_root().join(relative)
}

/// A C string that lives as long as the caller keeps the returned value.
pub fn c(text: &str) -> CString {
    CString::new(text).expect("test strings hold no NUL")
}

/// A live validator for `spec_id`, built against this repository's registry.
///
/// Panics rather than returning a `Result`: a test that cannot build a
/// validator has nothing left to say, and the last-error string is included so
/// the failure explains itself.
pub fn validator(spec_id: &str) -> *mut conform_ffi::ConformValidator {
    let spec = c(spec_id);
    let registry = c(&specs_toml().display().to_string());
    // SAFETY: both arguments are live, NUL-terminated C strings that outlive
    // the call.
    let handle = unsafe { abi::conform_validator_new(spec.as_ptr(), registry.as_ptr()) };
    assert!(
        !handle.is_null(),
        "could not build a `{spec_id}` validator: {}",
        error::last_error().unwrap_or_else(|| "<no explanation>".to_owned())
    );
    handle
}

/// Validate `document` and return the status alongside the JSON, if any.
///
/// Reads the length out-parameter as well, and checks it against the string's
/// own length — an out-parameter nobody verifies is an out-parameter that can
/// be wrong for years.
pub fn validate(
    handle: *mut conform_ffi::ConformValidator,
    document_id: &str,
    document: &[u8],
) -> (ConformStatus, Option<String>) {
    let id = c(document_id);
    let mut json: *mut c_char = std::ptr::null_mut();
    let mut length = usize::MAX;

    // SAFETY: a handle from `validator` above, a NUL-terminated id, a slice
    // that outlives the call, and two writable out-parameters.
    let status = unsafe {
        abi::conform_validate(
            handle,
            id.as_ptr(),
            document.as_ptr(),
            document.len(),
            &raw mut json,
            &raw mut length,
        )
    };

    if json.is_null() {
        assert_ne!(
            status,
            ConformStatus::Ok,
            "the call succeeded and handed back nothing"
        );
        assert_eq!(
            length, 0,
            "a failed call must zero the length out-parameter"
        );
        return (status, None);
    }

    // SAFETY: non-null, and by the API's contract a NUL-terminated string this
    // library owns until `conform_string_free` takes it back.
    let text = unsafe { CStr::from_ptr(json) }
        .to_str()
        .expect("the boundary promises UTF-8")
        .to_owned();
    assert_eq!(
        length,
        text.len(),
        "the length out-parameter disagrees with the string it describes"
    );

    // SAFETY: the pointer came from the call above and is freed exactly once.
    unsafe { abi::conform_string_free(json) };
    (status, Some(text))
}

/// Release a handle.
pub fn release(handle: *mut conform_ffi::ConformValidator) {
    // SAFETY: a live handle, freed exactly once, with no other thread using it.
    unsafe { abi::conform_validator_free(handle) };
}

/// This thread's last error, or a placeholder.
pub fn last_error() -> String {
    error::last_error().unwrap_or_else(|| "<none>".to_owned())
}

/// U+202E RIGHT-TO-LEFT OVERRIDE, an ANSI colour sequence, an OSC
/// window-title sequence terminated by BEL, and a zero-width space.
///
/// The same payload `conform-cli`'s `hostile_text_is_neutralised.rs` uses, so
/// the two crates are held to opposite halves of one rule with one string.
/// Written as escapes rather than literal bytes so that reading this file is
/// not itself an exercise in trusting your terminal.
pub const HOSTILE: &str = "act\u{202e}evi\u{1b}[31mtcani\u{1b}]0;pwned\u{7}ve\u{200b}d";

/// A minimal ODCS contract whose `status` is the payload.
pub fn hostile_contract() -> String {
    format!(
        "version: 1.0.0\n\
         apiVersion: v3.1.0\n\
         kind: DataContract\n\
         id: 53581432-6c55-4ba2-a65f-72344a91553a\n\
         status: {}\n",
        yaml_quoted(HOSTILE)
    )
}

/// The payload as a YAML double-quoted scalar.
///
/// Written through YAML's own `\uXXXX` escapes rather than as raw bytes,
/// because YAML forbids most control characters in a scalar and a parser that
/// rejected the fixture would make the test pass for the wrong reason. What
/// reaches the adapter is the real character either way.
fn yaml_quoted(text: &str) -> String {
    use std::fmt::Write as _;

    let mut quoted = String::from("\"");
    for character in text.chars() {
        if character.is_ascii_graphic() && character != '"' && character != '\\' {
            quoted.push(character);
        } else {
            let _ = write!(quoted, "\\u{:04X}", character as u32);
        }
    }
    quoted.push('"');
    quoted
}
