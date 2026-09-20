//! A C ABI over the conformance harness.
//!
//! Optional and entirely additive: nothing else in this repository depends on
//! this crate, and its absence changes nothing. What it adds is a way for a
//! program that is not Rust to ask the same question `conform validate` asks
//! — *does this document conform to this standard, and if not, where?* — and
//! get the same answer.
//!
//! # The shape of the API
//!
//! Six functions, plus one self-test:
//!
//! ```c
//! const char*        conform_version(void);
//! ConformValidator*  conform_validator_new(const char* spec_id,
//!                                          const char* registry_path);
//! ConformStatus      conform_validate(ConformValidator*,
//!                                     const char* document_id,
//!                                     const uint8_t* document, size_t len,
//!                                     char** out_json, size_t* out_len);
//! const char*        conform_last_error(void);
//! void               conform_string_free(char*);
//! void               conform_validator_free(ConformValidator*);
//! ConformStatus      conform_self_test_panic(void);
//! ```
//!
//! # The boundary speaks JSON, not structs
//!
//! No Rust type crosses. A validation returns the [`report::Envelope`] as a
//! JSON string, which is the decision the rest of the design follows from: the
//! ABI is a handful of signatures that never mention a diagnostic, so adding a
//! field to [`conform_core::Diagnostic`] — which will happen — is not an ABI
//! break for every downstream binding at once. It would be, and a *silent*
//! one, if a struct crossed instead. See [`report`] for the argument in full.
//!
//! # Nothing unwinds across the boundary
//!
//! Every entry point wraps its body in [`std::panic::catch_unwind`]; a panic
//! becomes [`ConformStatus::Panic`] and an explanation in
//! [`abi::conform_last_error`]. Unwinding through a C frame is undefined
//! behaviour, and a library that can do it is a library that can take its host
//! process down for a bug in a diagnostic message. [`abi::conform_self_test_panic`]
//! lets a binding prove the net is there in the build it linked.
//!
//! # Where the `unsafe` is
//!
//! In [`abi`], and nowhere else. The workspace forbids `unsafe_code`; this
//! crate downgrades that to `deny` in its own manifest and lifts it, in
//! writing, on exactly one module. [`engine`], [`report`], [`error`] and
//! [`status`] are ordinary safe Rust and are still held to `deny`, which is
//! what keeps the part of this crate that can be wrong in the dangerous way
//! down to one readable file.
//!
//! # The header
//!
//! `include/conform.h` is generated and committed, so that a C program needs
//! no Rust toolchain to find out what it is linking and so that an ABI change
//! shows up in a diff. The generator is `tools/headergen`, deliberately
//! outside the cargo workspace — `cbindgen` is MPL-2.0 and must not appear in
//! this workspace's dependency tree (plan §5.3, risk R4).
//! `tests/the_header_matches_the_library.rs` runs it and fails if the
//! committed copy has drifted; it is gated behind `--features header-check`
//! because it shells out to cargo. The C smoke test needs a C compiler and is
//! gated behind `--features c-smoke`. A plain `cargo test --workspace` needs
//! neither, which is the point.
//!
//! # Example, from Rust
//!
//! The `rlib` is built alongside the `cdylib`, so the entry points can be
//! called directly — which is how every test in this crate exercises them.
//!
//! ```no_run
//! use std::ffi::{CStr, CString};
//! use conform_ffi::{ConformStatus, abi};
//!
//! let spec = CString::new("odcs").unwrap();
//! let registry = CString::new("specs.toml").unwrap();
//! // SAFETY: both arguments are live, NUL-terminated C strings.
//! let validator = unsafe { abi::conform_validator_new(spec.as_ptr(), registry.as_ptr()) };
//! assert!(!validator.is_null());
//!
//! let id = CString::new("orders.yaml").unwrap();
//! let document = b"apiVersion: v3.0.2\n";
//! let mut json = std::ptr::null_mut();
//! let mut length = 0usize;
//! // SAFETY: a live handle, a NUL-terminated id, a readable slice, and two
//! // writable out-parameters.
//! let status = unsafe {
//!     abi::conform_validate(
//!         validator,
//!         id.as_ptr(),
//!         document.as_ptr(),
//!         document.len(),
//!         &raw mut json,
//!         &raw mut length,
//!     )
//! };
//! assert_eq!(status, ConformStatus::Ok);
//!
//! // SAFETY: `conform_validate` returned `Ok`, so `json` is a C string it owns.
//! let report = unsafe { CStr::from_ptr(json) }.to_str().unwrap().to_owned();
//! // SAFETY: the pointer came from the call above and is freed exactly once.
//! unsafe { abi::conform_string_free(json) };
//! // SAFETY: as above.
//! unsafe { abi::conform_validator_free(validator) };
//! ```

pub mod abi;
pub mod engine;
pub mod error;
pub mod report;
pub mod status;

pub use abi::ConformValidator;
pub use report::{Envelope, SCHEMA_VERSION};
pub use status::ConformStatus;

/// The version of the **ABI** — the function signatures, the status codes and
/// the ownership rules, not the library and not the JSON.
///
/// Emitted into `conform.h` as a compile-time constant so that a binding can
/// refuse a library it was not written against. It changes only when an
/// existing signature or an existing status code changes meaning; adding a
/// function or appending a status code does not move it, because neither
/// breaks a caller that does not use them.
///
/// The other two numbers a consumer may want are `conform_version()`, which is
/// this library's release version, and `CONFORM_SCHEMA_VERSION`, which is the
/// shape of the JSON that comes back.
pub const CONFORM_ABI_VERSION: u32 = 1;
