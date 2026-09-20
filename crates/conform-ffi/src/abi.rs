// The one opt-out from `unsafe_code`, and the reason this module exists as a
// module at all.
//
// `[workspace.lints.rust] unsafe_code = "forbid"` holds for the other six
// crates in this repository and must keep holding. A C ABI cannot be written
// without `unsafe`, so this crate's manifest sets `deny` instead of `forbid`
// — the difference being that `deny` can be lifted, in writing, at one place.
// This is that place, and it is the *only* one: `engine`, `report`, `error`
// and `status` are all still held to `deny`, and none of them contains the
// word. The dangerous surface of this crate is therefore exactly this file,
// and it is short on purpose.
#![allow(
    unsafe_code,
    reason = "this module is the C ABI; every unsafe block below is a \
              documented pointer contract, and no other module in the crate \
              is permitted one"
)]

//! The boundary itself.
//!
//! Seven `extern "C"` entry points, six of which are the API and one of which
//! is a self-test. Each is a shim over [`crate::engine`]: it turns pointers
//! into Rust values, calls into safe code, and turns what comes back into an
//! integer and a C string.
//!
//! # Panics never cross
//!
//! Every entry point below wraps its whole body in
//! [`std::panic::catch_unwind`]. An unwind through a C stack frame is
//! undefined behaviour — not "unsupported", not "messy": undefined — and the
//! only defence is to stop it on this side. A caught panic becomes
//! [`ConformStatus::Panic`] with the payload in the last-error slot, so the
//! bug is reported rather than either hidden or fatal.
//!
//! [`conform_self_test_panic`] exists so that a binding can prove this works
//! *in the library it actually linked*. It matters because the defence has a
//! prerequisite the linker does not check: a build with `panic = "abort"` has
//! no unwinding to catch, and `catch_unwind` cannot help. Calling that
//! function and getting `CONFORM_STATUS_PANIC` back is the proof; a process
//! that dies instead is the diagnosis.
//!
//! # What the handle guard can and cannot catch
//!
//! A [`ConformValidator`] carries a guard word at offset zero. Every call that
//! takes a handle checks that the pointer is non-null, correctly aligned, and
//! that the eight bytes it points at are that word. That turns three real
//! mistakes — a wild pointer into live data, a pointer to a zeroed or
//! uninitialised buffer, a handle freed by [`conform_validator_free`], which
//! poisons the word before releasing the allocation — from undefined behaviour
//! into [`ConformStatus::InvalidHandle`].
//!
//! It cannot catch a pointer into memory the process does not have mapped:
//! reading eight bytes there is a fault, and there is no portable way for any
//! library to test a pointer before following it. It also cannot *guarantee*
//! it catches a use-after-free, because by then the allocator may have reused
//! the bytes; the poisoning makes it likely, not certain. A guard word is a
//! smoke alarm, not a fire door, and this is what it is worth.
//!
//! # Thread safety
//!
//! A [`ConformValidator`] is `Send` but not `Sync`: move one between threads
//! freely, do not call into one from two threads at once. The last-error slot
//! is per thread, so two threads failing at once do not overwrite each other's
//! explanation.

use std::ffi::{CStr, CString, c_char};
use std::panic::AssertUnwindSafe;
use std::path::Path;

use crate::engine::{Backend, BuildError};
use crate::error;
use crate::status::ConformStatus;

/// The guard word a real handle carries at offset zero.
///
/// ASCII `conform`, NUL-padded. Built with `from_ne_bytes` rather than
/// `from_be_bytes` so that the *bytes in memory* spell it on every
/// architecture, which is what makes a handle recognisable in a hex dump; the
/// numeric value differs between a big- and a little-endian host and never
/// leaves the process, so that costs nothing.
///
/// Chosen so that the far likelier accidents — all-zero memory, a small
/// integer, a pointer — are not it.
const HANDLE_MAGIC: u64 = u64::from_ne_bytes(*b"conform\0");

/// The value written over the guard word when a handle is freed.
///
/// `from_ne_bytes`, for the reason above: what a debugger shows at that
/// address should read as `DEADconf` wherever this is built.
const HANDLE_POISON: u64 = u64::from_ne_bytes(*b"DEADconf");

/// An opaque handle to a built validator.
///
/// Created by [`conform_validator_new`], released by
/// [`conform_validator_free`], and meaningful to nothing else. Its layout is
/// deliberately not part of the ABI: a C caller only ever holds a pointer to
/// one, which is what the annotation on the last line of this comment tells
/// the header generator to emit.
///
/// cbindgen:opaque
#[repr(C)]
pub struct ConformValidator {
    /// The guard word. First field, and `repr(C)`, so that it is at offset
    /// zero and the probe below can read it without knowing anything else
    /// about what it was handed.
    magic: u64,
    /// The validator proper. Boxed so this struct stays two words whatever
    /// the adapters grow into.
    backend: Box<Backend>,
}

// Opaque to C, so opaque here too: what a reader wants to know is which
// standard it speaks for and whether it is still live.
impl std::fmt::Debug for ConformValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConformValidator")
            .field("live", &(self.magic == HANDLE_MAGIC))
            .field("spec", &self.backend.spec().id)
            .finish_non_exhaustive()
    }
}

/// Run an entry point's body with a panic net under it.
///
/// `on_panic` is what the entry point returns if the body unwinds — a null
/// pointer, a status code, or nothing, depending on the signature.
///
/// [`AssertUnwindSafe`] is correct rather than convenient here. The state the
/// body can touch across an unwind is the caller's memory, which this library
/// does not own and cannot reason about anyway, and the thread-local
/// last-error slot, which is overwritten unconditionally on the way out. There
/// is no invariant of this library's own left half-updated by an unwind,
/// because every entry point's Rust state is local to the call.
fn boundary<T>(entry: &'static str, on_panic: T, body: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(payload) => {
            error::set(format!("{entry}: panicked: {}", describe(payload.as_ref())));
            on_panic
        }
    }
}

/// Make something readable out of a panic payload.
fn describe(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "a panic payload of a type this library cannot read".to_owned()
    }
}

/// Borrow a handle, refusing anything that is not one.
///
/// # Safety
///
/// `validator` must either be null or point at memory the process has mapped
/// for at least eight bytes. Everything else — alignment, whether those bytes
/// are a handle at all — is checked here rather than assumed.
unsafe fn borrow<'a>(validator: *mut ConformValidator) -> Result<&'a Backend, ConformStatus> {
    if validator.is_null() {
        error::set("the validator handle was null");
        return Err(ConformStatus::NullPointer);
    }
    if !validator
        .addr()
        .is_multiple_of(align_of::<ConformValidator>())
    {
        error::set("the validator handle is not a handle: the pointer is misaligned");
        return Err(ConformStatus::InvalidHandle);
    }
    // SAFETY: non-null and aligned, and the caller's half of the contract is
    // that it is readable. `read_unaligned` rather than a field access so that
    // this probe never itself assumes the bytes are a `ConformValidator` — it
    // is the check that decides whether they are.
    let magic = unsafe { validator.cast::<u64>().read_unaligned() };
    if magic != HANDLE_MAGIC {
        error::set(if magic == HANDLE_POISON {
            "the validator handle has already been freed"
        } else {
            "the validator handle is not a handle: its guard word is wrong"
        });
        return Err(ConformStatus::InvalidHandle);
    }
    // SAFETY: the guard word matched, so these bytes were written by
    // `conform_validator_new` and not yet freed. The returned lifetime is
    // unbounded, which is the caller's obligation: the handle must outlive the
    // call, and `conform_validator_free` must not run concurrently with it.
    // Both are documented on every entry point that takes a handle.
    Ok(&unsafe { &*validator }.backend)
}

/// Read a C string argument.
///
/// # Safety
///
/// `pointer` must either be null or point at a NUL-terminated byte string.
unsafe fn c_str<'a>(pointer: *const c_char, what: &str) -> Result<&'a str, ConformStatus> {
    if pointer.is_null() {
        error::set(format!("{what} was null"));
        return Err(ConformStatus::NullPointer);
    }
    // SAFETY: non-null, and NUL-terminated by the caller's contract.
    let bytes = unsafe { CStr::from_ptr(pointer) };
    bytes.to_str().map_err(|error| {
        error::set(format!("{what} is not valid UTF-8: {error}"));
        ConformStatus::InvalidUtf8
    })
}

/// This library's version, as a NUL-terminated string.
///
/// The NUL is appended at compile time so the constant *is* a C string and
/// nothing has to allocate one.
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// The version of this library, as a NUL-terminated UTF-8 string.
///
/// Static storage. The caller must **not** free it, and it is valid for the
/// life of the process.
///
/// This is the *library's* version and moves when the crate is released. The
/// version of the **ABI** is the compile-time constant `CONFORM_ABI_VERSION`
/// in this header, and the version of the JSON a validation returns is the
/// `schema_version` field inside it. They are three different numbers because
/// they answer three different questions.
#[unsafe(no_mangle)]
pub extern "C" fn conform_version() -> *const c_char {
    boundary("conform_version", std::ptr::null(), || {
        VERSION.as_ptr().cast::<c_char>()
    })
}

/// Build a validator for one specification.
///
/// `spec_id` is a registry identifier — `"odcs"` or `"odps"`. `registry_path`
/// is the path to a `specs.toml`; it is **required**, because this library
/// does not go looking through an embedder's filesystem for one. The schema
/// behind the entry is re-hashed against the digest the registry records
/// before the validator is handed back, so a drifted schema is a failure here
/// rather than a quiet verdict later.
///
/// Returns null on failure, with the reason in [`conform_last_error`].
/// Release the handle with [`conform_validator_free`].
///
/// # Safety
///
/// Both arguments must either be null or point at NUL-terminated byte
/// strings, and must stay readable for the duration of the call. Nothing is
/// borrowed beyond it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn conform_validator_new(
    spec_id: *const c_char,
    registry_path: *const c_char,
) -> *mut ConformValidator {
    boundary("conform_validator_new", std::ptr::null_mut(), || {
        error::clear();

        // SAFETY: forwarded from this function's own safety contract.
        let Ok(spec_id) = (unsafe { c_str(spec_id, "the spec id") }) else {
            return std::ptr::null_mut();
        };
        // SAFETY: as above.
        let Ok(registry_path) = (unsafe { c_str(registry_path, "the registry path") }) else {
            return std::ptr::null_mut();
        };

        match Backend::build(spec_id, Path::new(registry_path)) {
            Ok(backend) => Box::into_raw(Box::new(ConformValidator {
                magic: HANDLE_MAGIC,
                backend: Box::new(backend),
            })),
            Err(error @ (BuildError::UnknownSpec(_) | BuildError::Registry(_))) => {
                error::set(error.message());
                std::ptr::null_mut()
            }
        }
    })
}

/// Validate one document and return the report as JSON.
///
/// `document` and `document_len` are the document's bytes; they must be UTF-8
/// and need not be NUL-terminated. `document` may be null only when
/// `document_len` is zero. `document_id` is the name the findings will locate
/// themselves by — a path, a URL, or anything else meaningful to the caller.
///
/// On success `*out_json` receives a NUL-terminated UTF-8 JSON string owned by
/// this library; release it with [`conform_string_free`]. If `out_len` is not
/// null it receives the string's length in bytes, excluding the NUL, so a
/// caller need not walk it. On failure `*out_json` is set to null and
/// `*out_len`, if given, to zero.
///
/// **A document full of errors is a success.** The return value says whether a
/// report was produced; what the report says is in the JSON. Reporting and
/// gating are separate decisions in this family, and this boundary does not
/// gate at all — the caller applies whatever policy it has to the severities
/// it reads.
///
/// The JSON is JSON-encoded and is **not** escaped for a terminal. A message
/// quotes the document verbatim, so it can carry a bidirectional override or
/// an ANSI sequence; neutralising that is the job of whatever displays it, and
/// doing it here as well would corrupt the data.
///
/// # Safety
///
/// `validator` must be a live handle from [`conform_validator_new`] that no
/// other thread is using concurrently. `document_id` must be a NUL-terminated
/// byte string. `document` must be readable for `document_len` bytes.
/// `out_json` must be a writable pointer to a `char*`, and `out_len`, if not
/// null, a writable pointer to a `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn conform_validate(
    validator: *mut ConformValidator,
    document_id: *const c_char,
    document: *const u8,
    document_len: usize,
    out_json: *mut *mut c_char,
    out_len: *mut usize,
) -> ConformStatus {
    boundary("conform_validate", ConformStatus::Panic, || {
        error::clear();

        if out_json.is_null() {
            error::set("`out_json` was null, so there is nowhere to put the report");
            return ConformStatus::NullPointer;
        }
        // Cleared *first*, so that a caller who ignores the return value and
        // frees `*out_json` unconditionally frees a null rather than whatever
        // was in that variable beforehand.
        // SAFETY: checked non-null immediately above; writability is the
        // caller's half of the contract.
        unsafe { out_json.write(std::ptr::null_mut()) };
        if !out_len.is_null() {
            // SAFETY: checked non-null; writability is the caller's contract.
            unsafe { out_len.write(0) };
        }

        // SAFETY: forwarded from this function's own safety contract.
        let backend = match unsafe { borrow(validator) } {
            Ok(backend) => backend,
            Err(status) => return status,
        };
        // SAFETY: as above.
        let document_id = match unsafe { c_str(document_id, "the document id") } {
            Ok(id) => id,
            Err(status) => return status,
        };

        if document.is_null() && document_len != 0 {
            error::set("the document pointer was null but its length was not zero");
            return ConformStatus::NullPointer;
        }
        let bytes: &[u8] = if document_len == 0 {
            &[]
        } else {
            // SAFETY: non-null, and readable for `document_len` bytes by the
            // caller's contract. The slice is not held beyond this call.
            unsafe { std::slice::from_raw_parts(document, document_len) }
        };

        let Ok(text) = std::str::from_utf8(bytes) else {
            error::set(
                "the document is not valid UTF-8; both standards this library validates are \
                 text, so there is nothing here that could be checked",
            );
            return ConformStatus::InvalidUtf8;
        };

        let json = match backend.validate_to_json(document_id, text) {
            Ok(json) => json,
            Err(error) => {
                error::set(format!("the report could not be serialised: {error}"));
                return ConformStatus::Serialisation;
            }
        };
        let length = json.len();

        // `serde_json` escapes a NUL byte as a six-character escape sequence, so this cannot
        // fail for a serialised envelope. Handled rather than unwrapped
        // because a panic in an FFI boundary is a worse outcome than a status
        // code, always.
        let Ok(encoded) = CString::new(json) else {
            error::set("the report held a NUL byte and cannot be handed over as a C string");
            return ConformStatus::Serialisation;
        };

        // SAFETY: checked non-null at the top; writability is the caller's
        // contract. Ownership of the allocation passes to the caller here, and
        // `conform_string_free` is how it comes back.
        unsafe { out_json.write(encoded.into_raw()) };
        if !out_len.is_null() {
            // SAFETY: checked non-null above.
            unsafe { out_len.write(length) };
        }
        ConformStatus::Ok
    })
}

/// Why the last call on **this thread** failed, or null if it did not.
///
/// The string is owned by this library and must not be freed. It is valid
/// until the next call into this library on the same thread, and until the
/// thread exits; copy it if you need it for longer. This is `strerror`'s
/// contract, deliberately.
#[unsafe(no_mangle)]
pub extern "C" fn conform_last_error() -> *const c_char {
    boundary(
        "conform_last_error",
        std::ptr::null(),
        error::last_error_ptr,
    )
}

/// Release a string this library returned.
///
/// Null is accepted and does nothing, so a caller can free unconditionally.
///
/// # Safety
///
/// `text` must be null, or a pointer this library returned through `out_json`
/// and has not already taken back. Freeing anything else, or the same pointer
/// twice, is undefined behaviour that no guard word can catch: the allocation
/// carries no header this library controls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn conform_string_free(text: *mut c_char) {
    boundary("conform_string_free", (), || {
        if text.is_null() {
            return;
        }
        // SAFETY: by the contract above, this pointer came from
        // `CString::into_raw` in `conform_validate` and has not been freed.
        drop(unsafe { CString::from_raw(text) });
    });
}

/// Release a validator handle.
///
/// Null is accepted and does nothing. The guard word is poisoned before the
/// allocation is released, so a subsequent call with the same pointer is
/// *likely* — not certainly; see this module's documentation — to be refused
/// with `CONFORM_STATUS_INVALID_HANDLE` rather than to be undefined.
///
/// # Safety
///
/// `validator` must be null, or a live handle from [`conform_validator_new`]
/// that has not already been freed and that no other thread is using.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn conform_validator_free(validator: *mut ConformValidator) {
    boundary("conform_validator_free", (), || {
        // SAFETY: forwarded from this function's own safety contract. The
        // probe refuses anything that is not a live handle, so a double free
        // through this entry point lands here rather than in the allocator.
        if unsafe { borrow(validator) }.is_err() {
            return;
        }
        // SAFETY: the probe just confirmed the guard word, so these bytes came
        // from `Box::into_raw` in `conform_validator_new`. Poison first: after
        // the box is dropped the allocation may be reused, and the window in
        // which the poison is readable is the whole value of writing it.
        unsafe { validator.cast::<u64>().write(HANDLE_POISON) };
        // SAFETY: as above.
        drop(unsafe { Box::from_raw(validator) });
    });
}

/// Prove that a panic inside this library cannot escape it.
///
/// Panics on purpose, catches it, and returns `CONFORM_STATUS_PANIC`. A
/// binding should call this once at start-up: getting that code back is proof
/// that the library it linked was built with unwinding and that the net under
/// every other entry point is real. A process that *dies* here has linked a
/// `panic = "abort"` build, in which case no entry point in this library is
/// safe to call from C and the only fix is to rebuild it.
///
/// It writes a message to `stderr` on the way past, because that is what a
/// Rust panic does and suppressing it would mean installing a process-global
/// panic hook from a library, which is not this library's to install.
///
/// # Panics
///
/// It panics deliberately, and catches it. Nothing escapes; if something
/// did, the message in this section would be the last thing the host
/// process read, which is the whole reason this function exists.
#[unsafe(no_mangle)]
pub extern "C" fn conform_self_test_panic() -> ConformStatus {
    boundary("conform_self_test_panic", ConformStatus::Panic, || {
        error::clear();
        panic!("deliberate panic from conform_self_test_panic");
    })
}
