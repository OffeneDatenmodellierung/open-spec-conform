//! The last error, per thread.
//!
//! # Why the slot is not on the handle
//!
//! The obvious place for a last-error string is the validator handle, and it
//! is the wrong one: the call most likely to fail is the one that *creates*
//! the handle, and it has no handle to leave a message on. A caller who gets
//! a null back from the constructor needs to be told why, so the slot has to
//! outlive — and pre-date — any handle.
//!
//! # Why per thread and not global
//!
//! Two threads validating two documents must not overwrite each other's
//! explanation of what went wrong, and a mutex would hand a caller a message
//! that some other thread wrote. A thread-local slot gives the one guarantee
//! a C caller can actually use: *the message describes the call you just
//! made, on the thread you made it on*.
//!
//! # The lifetime contract, stated once
//!
//! The pointer [`last_error_ptr`] returns is owned by this library and must
//! not be freed. It stays valid until the next call into this library **on
//! the same thread**, and until that thread exits. A caller that needs it for
//! longer must copy it. This is the same contract as `strerror`, and it is
//! stated on every entry point that can fail.
//!
//! No `unsafe` appears in this module. Producing a pointer is safe; only
//! following one is not, and following one is the caller's business.

use std::cell::RefCell;
use std::ffi::{CString, c_char};

thread_local! {
    /// The explanation of this thread's most recent failure, if it had one.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Record why the call this thread is in the middle of failed.
///
/// A NUL byte in the message would make it unrepresentable as a C string.
/// Rather than losing the message, the NUL is replaced: a slightly mangled
/// explanation beats no explanation, and the only way a NUL gets in here is a
/// document that put one in a value we quoted back.
pub fn set(message: impl Into<String>) {
    let message = message.into().replace('\0', "\u{fffd}");
    let encoded = CString::new(message).unwrap_or_else(|_| {
        // Unreachable: every NUL was just removed. Written as a fallback
        // rather than an `expect` because a panic in the error path of an FFI
        // boundary is the least useful panic there is.
        CString::default()
    });
    LAST_ERROR.with(|slot| slot.replace(Some(encoded)));
}

/// Forget this thread's last error.
///
/// Called at the start of every fallible entry point, so that a caller who
/// reads the slot after a *successful* call gets a null rather than a stale
/// message about something that already went right since.
pub fn clear() {
    LAST_ERROR.with(|slot| slot.replace(None));
}

/// A pointer to this thread's last error, or null if it has none.
///
/// See this module's documentation for how long it stays valid.
#[must_use]
pub fn last_error_ptr() -> *const c_char {
    LAST_ERROR.with(|slot| match &*slot.borrow() {
        // The `CString` stays in the slot; only the borrow ends here. The
        // pointer is invalidated by the next `set` or `clear` on this thread,
        // which is exactly the documented contract.
        Some(message) => message.as_ptr(),
        None => std::ptr::null(),
    })
}

/// This thread's last error as a Rust string, for tests and for Rust callers
/// who reach the library through the `rlib`.
#[must_use]
pub fn last_error() -> Option<String> {
    LAST_ERROR.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|message| message.to_string_lossy().into_owned())
    })
}
