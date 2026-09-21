//! What a call across the boundary can say about itself.
//!
//! # Why a status code and not a reported diagnostic
//!
//! Everything this library has to say about a *document* is a
//! [`Diagnostic`](conform_core::Diagnostic), and crosses as JSON. Nothing in
//! this enumeration is ever about a document: these are the failures of the
//! *call* — a pointer that was null, a handle that is not one, a spec id with
//! no adapter behind it. The distinction is the same one `conform-cli` draws
//! between a run that reached a verdict and a run that could not be performed,
//! and it matters more here, because a caller in C has no other way to tell
//! "this contract is broken" from "you passed me nothing".
//!
//! A document full of errors therefore returns [`ConformStatus::Ok`] and a
//! JSON report listing them. That is not a mistake; it is the same rule as
//! `conform validate` exiting 0 with findings.
//!
//! # Stability
//!
//! The numeric values are part of the ABI. New variants may be appended with
//! new numbers; an existing number never changes meaning. A binding that does
//! not recognise a code must treat any non-zero value as a failure, which is
//! why `Ok` is zero and nothing else is.

/// The outcome of a call across the boundary.
///
/// `0` is success. Every other value is a failure of the call itself, and
/// leaves a human-readable explanation in `conform_last_error`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConformStatus {
    /// The call did what it was asked. For a validation this means a report
    /// was produced — **not** that the document conformed.
    Ok = 0,

    /// A pointer the call cannot do without was null.
    NullPointer = 1,

    /// A handle was not one: it failed the guard word every validator carries.
    ///
    /// The guard catches null, misaligned, zeroed or freed handles. It cannot
    /// catch a pointer into memory the process has not mapped; nothing can.
    InvalidHandle = 2,

    /// A string argument was not valid UTF-8, or a C string was not
    /// terminated within the bounds the caller gave.
    InvalidUtf8 = 3,

    /// The spec id names no adapter this library can build.
    UnknownSpec = 4,

    /// The registry could not be loaded, or the vendored schema behind the
    /// requested spec failed its provenance check.
    ///
    /// The diagnostics explaining it are in the last-error string.
    Registry = 5,

    /// The report could not be serialised, or held a NUL byte and so could not
    /// be handed over as a C string.
    Serialisation = 6,

    /// A panic was caught at the boundary and did not cross it.
    ///
    /// Always a bug in this library. Reported rather than aborted because an
    /// unwind through a C frame is undefined behaviour and a returned code is
    /// not.
    Panic = 7,
}

impl ConformStatus {
    /// The variant's name, for a message or a log line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NullPointer => "null pointer",
            Self::InvalidHandle => "invalid handle",
            Self::InvalidUtf8 => "invalid utf-8",
            Self::UnknownSpec => "unknown spec",
            Self::Registry => "registry",
            Self::Serialisation => "serialisation",
            Self::Panic => "panic",
        }
    }

    /// Whether the call succeeded.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

impl std::fmt::Display for ConformStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
