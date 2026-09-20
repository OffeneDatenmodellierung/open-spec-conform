//! Where a diagnostic happened, described without knowing the document's shape.

use core::fmt;

/// Opaque identifier for one document in a set.
///
/// This crate never interprets the string. An adapter may use a file path, a
/// URI, a content hash or a synthetic name — whatever identifies a document
/// unambiguously within the set it was loaded from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct DocumentId(String);

impl DocumentId {
    /// Wrap an identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as it was given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for DocumentId {
    fn from(id: String) -> Self {
        Self::new(id)
    }
}

impl From<&str> for DocumentId {
    fn from(id: &str) -> Self {
        Self::new(id)
    }
}

/// Opaque pointer to a place *inside* a document.
///
/// Typically a JSON Pointer (`/schema/0/properties/id`) or a dotted field path,
/// but this crate imposes neither — an adapter writes whatever its own users
/// will recognise, and tooling treats it as a display string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Pointer(String);

impl Pointer {
    /// Wrap a pointer.
    #[must_use]
    pub fn new(pointer: impl Into<String>) -> Self {
        Self(pointer.into())
    }

    /// The pointer as it was given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for Pointer {
    fn from(pointer: String) -> Self {
        Self::new(pointer)
    }
}

impl From<&str> for Pointer {
    fn from(pointer: &str) -> Self {
        Self::new(pointer)
    }
}

/// Where a [`Diagnostic`](crate::Diagnostic) applies.
///
/// A document is always named; everything finer is optional, because how
/// precisely a position can be described depends on the adapter's parser, not
/// on this crate. A validator that can only say "somewhere in this document"
/// is still a valid producer of diagnostics.
///
/// ```
/// use conform_core::Location;
///
/// let coarse = Location::document("catalogue/orders");
/// let precise = Location::document("catalogue/orders")
///     .with_line(42)
///     .with_column(7)
///     .with_pointer("/fields/3/type");
///
/// assert_eq!(coarse.to_string(), "catalogue/orders");
/// assert_eq!(precise.to_string(), "catalogue/orders:42:7 (/fields/3/type)");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Location {
    /// Which document.
    pub document: DocumentId,
    /// One-based line, when the adapter's parser can say.
    pub line: Option<u32>,
    /// One-based column, when the adapter's parser can say.
    pub column: Option<u32>,
    /// Structural pointer within the document, when the adapter's parser can
    /// say.
    pub pointer: Option<Pointer>,
}

impl Location {
    /// A location naming a whole document and nothing finer.
    #[must_use]
    pub fn document(id: impl Into<DocumentId>) -> Self {
        Self {
            document: id.into(),
            line: None,
            column: None,
            pointer: None,
        }
    }

    /// Narrow this location to a line.
    #[must_use]
    pub const fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Narrow this location to a column.
    #[must_use]
    pub const fn with_column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    /// Narrow this location to a structural pointer.
    #[must_use]
    pub fn with_pointer(mut self, pointer: impl Into<Pointer>) -> Self {
        self.pointer = Some(pointer.into());
        self
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.document.as_str())?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
            if let Some(column) = self.column {
                write!(f, ":{column}")?;
            }
        }
        if let Some(pointer) = &self.pointer {
            write!(f, " ({pointer})")?;
        }
        Ok(())
    }
}
