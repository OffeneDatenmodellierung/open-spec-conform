//! Reference to the published standard a diagnostic was raised under.

use core::fmt;

/// Identifies the standard — and, where known, the exact version and clause —
/// that a [`Diagnostic`](crate::Diagnostic) or a
/// [`Validator`](crate::Validator) is speaking for.
///
/// This crate holds no opinion about what any `id` means; it is an opaque
/// string owned by whichever adapter or registry issued it. Carrying it on
/// every diagnostic is what lets downstream tooling answer "which version of
/// which standard said this was wrong?" without the adapter re-inventing a way
/// to say so.
///
/// ```
/// use conform_core::SpecRef;
///
/// let spec = SpecRef::new("example-standard")
///     .with_version("3.1.0")
///     .with_section("§4.2");
///
/// assert_eq!(spec.to_string(), "example-standard@3.1.0 §4.2");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecRef {
    /// Opaque identifier of the standard, as issued by the adapter or registry.
    pub id: String,
    /// The version of that standard, when the validator knows it.
    pub version: Option<String>,
    /// A clause, section or rule reference within that version, when the
    /// diagnostic can be pinned that precisely.
    pub section: Option<String>,
}

impl SpecRef {
    /// A reference naming a standard, with no version or section yet.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: None,
            section: None,
        }
    }

    /// Pin this reference to a version of the standard.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Pin this reference to a clause within the standard.
    #[must_use]
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }
}

impl fmt::Display for SpecRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)?;
        if let Some(version) = &self.version {
            write!(f, "@{version}")?;
        }
        if let Some(section) = &self.section {
            write!(f, " {section}")?;
        }
        Ok(())
    }
}
