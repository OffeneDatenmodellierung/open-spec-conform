//! The one trait an adapter implements to join this harness.

use crate::{ConformanceReport, GatePolicy, GateVerdict, SpecRef};

/// Validates documents of one kind against one published standard.
///
/// An adapter implements this over its **own** already-parsed document type.
/// Nothing here asks the adapter to change how it parses, what it parses into,
/// or which rules it runs — the trait sits strictly downstream of all of that.
///
/// The report-versus-gate split is built into the shape of the trait:
///
/// - [`validate`](Validator::validate) *reports*. It returns everything found
///   and cannot fail a run. There is no way for it to signal failure, so no
///   implementation can accidentally make reporting gate.
/// - [`check`](Validator::check) *gates*, against a policy the caller names.
///   Its default body is the whole convention: validate, then apply the
///   policy. An implementation should not need to override it.
///
/// ```
/// use conform_core::{
///     ConformanceReport, Diagnostic, GatePolicy, GateVerdict, Location, SpecRef, Validator,
/// };
///
/// struct NonEmpty;
///
/// impl Validator for NonEmpty {
///     type Document = str;
///
///     fn spec(&self) -> SpecRef {
///         SpecRef::new("example-standard").with_version("1.0")
///     }
///
///     fn validate(&self, document: &str) -> ConformanceReport {
///         let mut report = ConformanceReport::new();
///         if document.trim().is_empty() {
///             report.push(Diagnostic::error(
///                 "XYZ001",
///                 Location::document("<input>"),
///                 "document is empty",
///             ));
///         }
///         report
///     }
/// }
///
/// assert!(NonEmpty.validate("  ").should_gate(GatePolicy::default()));
/// assert_eq!(NonEmpty.check("ok", GatePolicy::default()), GateVerdict::Passed { worst: None });
/// ```
pub trait Validator {
    /// The adapter's own parsed document type. `?Sized` so a validator may
    /// take a `str`, a slice, or a trait object without a wrapper.
    type Document: ?Sized;

    /// Which standard, and which version of it, this validator speaks for.
    ///
    /// Adapters normally attach this same reference to the diagnostics they
    /// emit, via [`Diagnostic::with_spec_ref`](crate::Diagnostic::with_spec_ref).
    fn spec(&self) -> SpecRef;

    /// Report on a document. Always succeeds; never gates.
    fn validate(&self, document: &Self::Document) -> ConformanceReport;

    /// Report on a document, then apply a gating policy to the result.
    ///
    /// The default body is the convention this crate exists to share — override
    /// it only if "gating" genuinely means something different for a given
    /// standard, which it almost never does.
    fn check(&self, document: &Self::Document, policy: GatePolicy) -> GateVerdict {
        self.validate(document).gate(policy)
    }
}
