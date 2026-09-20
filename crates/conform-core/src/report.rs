//! Aggregating diagnostics, and the separate question of whether they gate.
//!
//! Reporting and gating are deliberately two decisions, not one. A validator
//! reports everything it found; a *policy* decides what any of that means for
//! an exit code. A command that fuses the two — that can only report by
//! failing, or can only pass by staying quiet — has been found wrong in
//! practice often enough that this crate makes the split structural.

use crate::{Diagnostic, Severity};

/// What a report is allowed to fail on.
///
/// The default gates on [`Severity::Error`] and nothing else, which is the
/// convention every consumer of this crate should inherit rather than restate.
///
/// ```
/// use conform_core::{GatePolicy, Severity};
///
/// assert_eq!(GatePolicy::default(), GatePolicy::AtOrAbove(Severity::Error));
/// assert_eq!(GatePolicy::report_only(), GatePolicy::Never);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GatePolicy {
    /// Never fail, whatever was found. Report-only: the bare validation call.
    Never,
    /// Fail if anything at or above this severity was found.
    AtOrAbove(Severity),
}

impl GatePolicy {
    /// Report everything, fail on nothing.
    #[must_use]
    pub const fn report_only() -> Self {
        Self::Never
    }

    /// Fail on errors only — the default.
    #[must_use]
    pub const fn errors_only() -> Self {
        Self::AtOrAbove(Severity::Error)
    }

    /// Fail on warnings as well as errors.
    #[must_use]
    pub const fn warnings_as_errors() -> Self {
        Self::AtOrAbove(Severity::Warning)
    }

    /// Whether a single severity trips this policy.
    #[must_use]
    pub const fn gates(self, severity: Severity) -> bool {
        match self {
            Self::Never => false,
            // Compared as discriminants rather than with `Ord`, only because
            // `Ord::ge` is not callable in a `const fn`. `Severity`'s variants
            // are declared in ascending order precisely so this is sound.
            Self::AtOrAbove(threshold) => (severity as u8) >= (threshold as u8),
        }
    }
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self::errors_only()
    }
}

/// The verdict of applying a [`GatePolicy`] to a [`ConformanceReport`].
///
/// Both variants carry the worst severity seen, because "passed, but there
/// were warnings" and "passed, nothing found" are different things to report
/// even though they exit the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GateVerdict {
    /// Nothing found that this policy fails on.
    Passed {
        /// The worst severity in the report, if it held anything at all.
        worst: Option<Severity>,
    },
    /// The policy failed on something.
    Gated {
        /// The worst severity in the report — necessarily present, and
        /// necessarily at or above the policy's threshold.
        worst: Severity,
    },
}

impl GateVerdict {
    /// Whether this verdict should fail the run.
    #[must_use]
    pub const fn is_gated(self) -> bool {
        matches!(self, Self::Gated { .. })
    }

    /// The worst severity the report held, gated or not.
    #[must_use]
    pub const fn worst(self) -> Option<Severity> {
        match self {
            Self::Passed { worst } => worst,
            Self::Gated { worst } => Some(worst),
        }
    }

    /// Conventional process exit code: `0` when passed, `1` when gated.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        if self.is_gated() { 1 } else { 0 }
    }
}

/// Every diagnostic one validation run produced.
///
/// ```
/// use conform_core::{ConformanceReport, Diagnostic, GatePolicy, GateVerdict, Location, Severity};
///
/// let mut report = ConformanceReport::new();
/// report.push(Diagnostic::warning("XYZ002", Location::document("orders"), "field name is shouty"));
///
/// // Reporting says something happened...
/// assert_eq!(report.len(), 1);
/// assert_eq!(report.worst_severity(), Some(Severity::Warning));
///
/// // ...and gating, separately, says it does not fail the run.
/// assert_eq!(report.gate(GatePolicy::default()), GateVerdict::Passed { worst: Some(Severity::Warning) });
/// assert!(!report.should_gate(GatePolicy::default()));
/// assert!(report.should_gate(GatePolicy::warnings_as_errors()));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ConformanceReport {
    diagnostics: Vec<Diagnostic>,
}

impl ConformanceReport {
    /// An empty report — a document with nothing to say about it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    /// Add one diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Absorb another report's diagnostics, in order.
    ///
    /// This is how a validator composes per-rule or per-document runs into one
    /// report without any of them needing to know about the others.
    pub fn merge(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// Every diagnostic, in the order it was produced.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// How many diagnostics the report holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Whether the report holds nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Iterate every diagnostic, in the order it was produced.
    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
    }

    /// Diagnostics of exactly this severity.
    pub fn by_severity(&self, severity: Severity) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(move |d| d.severity == severity)
    }

    /// Diagnostics of this severity or worse.
    pub fn at_or_above(&self, severity: Severity) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(move |d| d.severity >= severity)
    }

    /// How many diagnostics of exactly this severity.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.by_severity(severity).count()
    }

    /// The worst severity present, or `None` for an empty report.
    #[must_use]
    pub fn worst_severity(&self) -> Option<Severity> {
        self.diagnostics.iter().map(|d| d.severity).max()
    }

    /// Whether this report fails under the given policy.
    #[must_use]
    pub fn should_gate(&self, policy: GatePolicy) -> bool {
        self.worst_severity()
            .is_some_and(|worst| policy.gates(worst))
    }

    /// The full verdict: gated or not, and the worst severity either way.
    #[must_use]
    pub fn gate(&self, policy: GatePolicy) -> GateVerdict {
        match self.worst_severity() {
            Some(worst) if policy.gates(worst) => GateVerdict::Gated { worst },
            worst => GateVerdict::Passed { worst },
        }
    }
}

impl FromIterator<Diagnostic> for ConformanceReport {
    fn from_iter<I: IntoIterator<Item = Diagnostic>>(iter: I) -> Self {
        Self {
            diagnostics: iter.into_iter().collect(),
        }
    }
}

impl Extend<Diagnostic> for ConformanceReport {
    fn extend<I: IntoIterator<Item = Diagnostic>>(&mut self, iter: I) {
        self.diagnostics.extend(iter);
    }
}

impl IntoIterator for ConformanceReport {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}

impl<'a> IntoIterator for &'a ConformanceReport {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.iter()
    }
}
