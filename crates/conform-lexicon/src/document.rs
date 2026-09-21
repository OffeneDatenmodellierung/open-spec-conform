//! The document under test: some bytes, and a name to report them under.

use std::fs;
use std::io;
use std::path::Path;

use conform_core::DocumentId;

/// An ODCL document as it arrives — raw text, plus the name diagnostics about
/// it should carry.
///
/// Deliberately **unparsed**. Parsing is a thing that fails, and a failed parse
/// is a finding this crate has to be able to report through the same
/// [`ConformanceReport`](conform_core::ConformanceReport) as everything else.
/// A `Document` that could only be constructed from well-formed input would
/// push that failure back out to the caller as a second, differently-shaped
/// error channel — which is precisely the `Result<(), String>` habit this
/// crate exists to retire.
///
/// ```
/// use conform_lexicon::Document;
///
/// let document = Document::new("contracts/orders.yaml", "dataContractSpecification: 1.2.1\n");
/// assert_eq!(document.id().as_str(), "contracts/orders.yaml");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    id: DocumentId,
    text: String,
}

impl Document {
    /// A document with an explicit name.
    ///
    /// The name is opaque to this crate and appears verbatim in every
    /// diagnostic's [`Location`](conform_core::Location) — a path, a URL, or
    /// something synthetic like `<stdin>`.
    #[must_use]
    pub fn new(id: impl Into<DocumentId>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }

    /// Read a document from a file, naming it after the path it came from.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] if the file cannot be read. This is
    /// an `io::Result` and not a report on purpose: "I could not open the file
    /// you named" is a fact about the caller's filesystem, not a statement
    /// about a document's conformance, and conflating the two is how a missing
    /// file comes to be reported as a non-conformant contract.
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        Ok(Self::new(path.display().to_string(), text))
    }

    /// What this document is called in diagnostics.
    #[must_use]
    pub const fn id(&self) -> &DocumentId {
        &self.id
    }

    /// The document's raw text, exactly as it was handed over.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}
