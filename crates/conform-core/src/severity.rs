//! Diagnostic severity, and what severity means for a gate.

use core::fmt;

/// How serious a [`Diagnostic`](crate::Diagnostic) is.
///
/// The variants are declared in **ascending order of severity**, so the derived
/// [`Ord`] means what it reads like: `Info < Warning < Error`. That is what
/// makes `report.worst_severity()` a plain `max()` over the diagnostics, and it
/// is why the declaration order here is load-bearing rather than cosmetic.
///
/// Reporting and gating are separate concerns. A severity does not decide on
/// its own whether a run fails — a [`GatePolicy`](crate::GatePolicy) does, and
/// the default policy gates on [`Severity::Error`] alone.
///
/// ```
/// use conform_core::Severity;
///
/// assert!(Severity::Error > Severity::Warning);
/// assert!(Severity::Warning > Severity::Info);
/// assert_eq!(Severity::Error.to_string(), "error");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Severity {
    /// Something worth saying that is not a defect: a note, a statistic, a
    /// hint. Never gates under any policy other than one that asks for it
    /// explicitly.
    Info,
    /// A hygiene issue: the document is conformant, but something about it is
    /// questionable. Reported always, gates only under a policy that says so.
    Warning,
    /// A conformance failure. Gates under the default policy.
    Error,
}

impl Severity {
    /// Every severity, ascending.
    ///
    /// Useful for tabulating a report without hand-writing the list at each
    /// call site.
    pub const ALL: [Self; 3] = [Self::Info, Self::Warning, Self::Error];

    /// The lower-case, stable, machine-readable name of this severity.
    ///
    /// Stable in the same sense as a [`DiagnosticCode`](crate::DiagnosticCode):
    /// downstream tooling may match on these strings, so they change only with
    /// a major version.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Whether this severity gates by default.
    ///
    /// Only [`Severity::Error`] does. This is the single place that default
    /// encoded, so no consumer has to re-derive it.
    #[must_use]
    pub const fn gates_by_default(self) -> bool {
        matches!(self, Self::Error)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
