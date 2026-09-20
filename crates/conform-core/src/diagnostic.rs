//! A single finding, and the stable code that names it.

use core::fmt;

use crate::{Location, Severity, SpecRef};

/// A stable, machine-readable identifier for a *kind* of finding.
///
/// Stability is the contract: downstream tooling — dashboards, suppression
/// lists, CI annotations — matches on these strings, so an adapter changes the
/// code attached to a rule only with a major version of that adapter. The
/// message may be reworded freely; the code may not.
///
/// This crate imposes no syntax. An adapter picks a prefixed, numbered scheme
/// of its own (`XYZ001`) and documents it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    /// Wrap a code.
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// The code as it was given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for DiagnosticCode {
    fn from(code: String) -> Self {
        Self::new(code)
    }
}

impl From<&str> for DiagnosticCode {
    fn from(code: &str) -> Self {
        Self::new(code)
    }
}

/// One finding: what went wrong, how badly, and where.
///
/// ```
/// use conform_core::{Diagnostic, Location, Severity};
///
/// let d = Diagnostic::error("XYZ001", Location::document("orders").with_line(12), "required field is missing")
///     .with_help("add the field, or mark the document as a draft")
///     .with_spec_ref(conform_core::SpecRef::new("example-standard").with_version("3.1.0"));
///
/// assert_eq!(d.severity, Severity::Error);
/// assert_eq!(d.code.as_str(), "XYZ001");
/// assert_eq!(d.to_string(), "error[XYZ001] orders:12: required field is missing");
/// ```
///
/// The fields are public on purpose: this is a record, adapters construct
/// thousands of them, and an accessor per field would buy nothing. Adding a
/// field here is therefore a breaking change, which is the correct cost to pay
/// for a type this many crates pin against.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    /// How serious this finding is.
    pub severity: Severity,
    /// The stable code naming the kind of finding.
    pub code: DiagnosticCode,
    /// Human-readable explanation. Free to be reworded between versions.
    pub message: String,
    /// Where the finding applies.
    pub location: Location,
    /// What the reader might do about it, when the validator can say.
    pub help: Option<String>,
    /// The standard, version and clause the finding was raised under, when the
    /// validator knows.
    pub spec_ref: Option<SpecRef>,
}

impl Diagnostic {
    /// A diagnostic of an explicit severity.
    #[must_use]
    pub fn new(
        severity: Severity,
        code: impl Into<DiagnosticCode>,
        location: Location,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            location,
            help: None,
            spec_ref: None,
        }
    }

    /// A conformance failure. Gates under the default policy.
    #[must_use]
    pub fn error(
        code: impl Into<DiagnosticCode>,
        location: Location,
        message: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Error, code, location, message)
    }

    /// A hygiene issue. Reported always, gates only if asked for.
    #[must_use]
    pub fn warning(
        code: impl Into<DiagnosticCode>,
        location: Location,
        message: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Warning, code, location, message)
    }

    /// A note. Never a defect.
    #[must_use]
    pub fn info(
        code: impl Into<DiagnosticCode>,
        location: Location,
        message: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Info, code, location, message)
    }

    /// Attach a suggested remedy.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Attach the standard reference this finding was raised under.
    #[must_use]
    pub fn with_spec_ref(mut self, spec_ref: SpecRef) -> Self {
        self.spec_ref = Some(spec_ref);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] {}: {}",
            self.severity, self.code, self.location, self.message
        )
    }
}
