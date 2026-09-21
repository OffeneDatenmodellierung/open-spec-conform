//! The work, with no pointers in sight.
//!
//! Everything this crate actually *does* lives here, in ordinary safe Rust
//! with ordinary Rust types. [`crate::abi`] is a shim over this module and
//! nothing more: it converts pointers into `&str` and `&[u8]`, calls in here,
//! and converts what comes back into a C string and an integer. Keeping the
//! two apart is what makes the dangerous half small enough to read in one
//! sitting, and it is why every test below can exercise the real logic
//! without a raw pointer anywhere.
//!
//! # Which adapters are reachable, and why not all of them
//!
//! `odcs` and `odps` are here. `okf` is not, and its absence is a design
//! decision rather than an oversight: an OKF bundle is a *directory* of
//! cross-referencing Markdown files, and `conform_okf::load` takes a
//! filesystem root. A boundary whose whole premise is "bytes in" has nothing
//! to hand it. Inventing a tar-shaped or manifest-shaped encoding for a
//! directory would be this crate designing a bundle format, which is a
//! different crate's job and a much larger decision than an FFI should make on
//! its own. Asking for `okf` therefore fails loudly, with a message that says
//! this, rather than silently returning an empty report.
//!
//! # Provenance is not optional here either
//!
//! Both validators are built through [`Registry`], so the vendored schema is
//! re-hashed against the digest `specs.toml` records for it *before* any
//! verdict is issued against it. An embedder on the other side of a C ABI
//! cannot check that for itself, which makes it more important here than
//! anywhere else, not less.

use std::fmt;
use std::path::Path;

use conform_core::{ConformanceReport, SpecRef, Validator as _};
use conform_odcs::OdcsValidator;
use conform_odps::OdpsValidator;
use conform_registry::Registry;

use crate::report::Envelope;

/// The spec ids this crate can build a validator for.
///
/// Listed once, so the error message for an unknown id cannot drift away from
/// the match that produces it.
pub const REACHABLE_SPEC_IDS: [&str; 2] = ["odcs", "odps"];

/// The spec id of the bundle-shaped standard that deliberately is not
/// reachable across a bytes-in boundary. See this module's documentation.
const BUNDLE_SPEC_ID: &str = "okf";

/// One built validator, ready to be asked about documents.
pub enum Backend {
    /// Open Data Contract Standard.
    Odcs(Box<OdcsValidator>),
    /// Open Data Product Standard.
    Odps(Box<OdpsValidator>),
}

// Written by hand rather than derived: neither adapter is `Debug`, because
// both hold a compiled `jsonschema::Validator` that is not. What a reader
// wants from this anyway is which standard it speaks for.
impl fmt::Debug for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backend")
            .field("spec", &self.spec().id)
            .finish_non_exhaustive()
    }
}

impl Backend {
    /// Build the validator for `spec_id`, resolving its schema through the
    /// registry at `registry_path`.
    ///
    /// The id is checked before the registry is opened, so asking for
    /// something this library cannot do is answered by saying so, rather than
    /// by a filesystem complaint about a path that was never the problem.
    ///
    /// # Errors
    ///
    /// [`BuildError::UnknownSpec`] if nothing here speaks for that id;
    /// [`BuildError::Registry`] if the registry will not load, holds no such
    /// entry, or the vendored schema no longer hashes to its recorded digest.
    pub fn build(spec_id: &str, registry_path: &Path) -> Result<Self, BuildError> {
        if !REACHABLE_SPEC_IDS.contains(&spec_id) {
            return Err(unknown_spec(spec_id));
        }

        // Loaded here rather than through each adapter's `from_registry_path`
        // so that "the registry itself will not load" stays a distinct message
        // from "the registry loaded, and the schema behind this entry did not
        // survive its provenance check". A caller on the far side of a C ABI
        // has only the string to go on, so the two must not read alike.
        let registry = Registry::load_path(registry_path)
            .map_err(|error| BuildError::Registry(render(&error.into_report())))?;

        match spec_id {
            "odcs" => OdcsValidator::from_registry(&registry)
                .map(|validator| Self::Odcs(Box::new(validator)))
                .map_err(|error| BuildError::Registry(render(error.report()))),
            "odps" => OdpsValidator::from_registry(&registry)
                .map(|validator| Self::Odps(Box::new(validator)))
                .map_err(|error| BuildError::Registry(render(error.report()))),
            // Unreachable: `REACHABLE_SPEC_IDS` guards the door above. Written
            // as a real arm rather than an `unreachable!`, because a panic in
            // the one crate whose premise is that panics do not escape would
            // be a poor joke.
            other => Err(unknown_spec(other)),
        }
    }

    /// Which standard, and which version of it, this validator speaks for.
    #[must_use]
    pub fn spec(&self) -> SpecRef {
        match self {
            Self::Odcs(validator) => validator.spec(),
            Self::Odps(validator) => validator.spec(),
        }
    }

    /// Report on a document given by name and text. Always succeeds; never
    /// gates.
    #[must_use]
    pub fn validate(&self, document_id: &str, text: &str) -> ConformanceReport {
        match self {
            Self::Odcs(validator) => validator.validate_text(document_id, text),
            Self::Odps(validator) => validator.validate_text(document_id, text),
        }
    }

    /// Report on a document and serialise the report as the boundary's
    /// envelope.
    ///
    /// # Errors
    ///
    /// Only if `serde_json` cannot serialise the envelope, which for these
    /// types means an allocation failure or a non-finite number, neither of
    /// which this shape can hold. It is propagated rather than unwrapped
    /// because an `unwrap` inside a `catch_unwind` is a panic reported as a
    /// bug, and this one is not a bug.
    pub fn validate_to_json(
        &self,
        document_id: &str,
        text: &str,
    ) -> Result<String, serde_json::Error> {
        let report = self.validate(document_id, text);
        serde_json::to_string(&Envelope::of(document_id, &self.spec(), &report))
    }
}

/// Why a validator could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// No adapter here speaks for that spec id.
    UnknownSpec(String),
    /// The registry, or the schema behind it, would not have it.
    Registry(String),
}

impl BuildError {
    /// The explanation, ready to be put in the last-error slot.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::UnknownSpec(message) | Self::Registry(message) => message,
        }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for BuildError {}

/// The refusal for a spec id this library cannot build a validator for.
///
/// `okf` gets its own sentence because it is the one id a reader will
/// reasonably expect to work — it is in the registry, the workspace has an
/// adapter for it, and the `conform` binary validates it happily. What it does
/// not have is a document; see this module's opening note.
fn unknown_spec(spec_id: &str) -> BuildError {
    if spec_id == BUNDLE_SPEC_ID {
        return BuildError::UnknownSpec(format!(
            "`{BUNDLE_SPEC_ID}` is a bundle of files on disk, not a single document, so it cannot \
             be validated through a boundary that takes a document's bytes; use the `conform` \
             binary or the `conform-okf` crate for a bundle"
        ));
    }
    BuildError::UnknownSpec(format!(
        "`{spec_id}` names no validator this library can build; it knows {}",
        REACHABLE_SPEC_IDS.join(" and ")
    ))
}

/// Flatten an adapter's refusal into one string for the last-error slot.
///
/// The diagnostics are kept verbatim, codes and all, rather than paraphrased:
/// a caller reading `REG002` on the other side of a C ABI can look it up, and
/// "the schema was rejected" cannot be looked up at all.
///
/// `pub(crate)` rather than private so that [`crate::embedded`] flattens a
/// refusal the same way this module does. Two spellings of "here is what the
/// registry said" is exactly the drift the rest of this repository spends its
/// tests preventing.
pub(crate) fn render(report: &ConformanceReport) -> String {
    if report.is_empty() {
        return "the validator could not be built, and said nothing about why".to_owned();
    }
    report
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
