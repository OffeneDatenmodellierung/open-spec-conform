//! Finding the things to check, and working out what each of them is.
//!
//! Two questions, and neither of them is guesswork about *conformance*:
//!
//! 1. **Which files?** A path given on the command line may be one document or
//!    a directory holding many. Walking it is this module's job.
//! 2. **Which standard?** An ODCS contract and an ODPS product are both YAML
//!    with an `apiVersion`, and the file extension does not distinguish them.
//!    One key does: `kind`. An ODCL document carries no `kind` at all; what it
//!    carries is a root `dataContractSpecification`, which its schema makes
//!    required. Each standard is routed by the discriminator its own
//!    specification defines, and never by one borrowed from a sibling.
//!
//! # Sniffing is a routing decision, never a verdict
//!
//! The distinction matters, because sniffing has already caused a defect in
//! this estate. `validate_odcs_internal` sniffed its input and, on finding a
//! `dataContractSpecification` key, silently validated the document against a
//! *different* schema and reported the result as an ODCS verdict — so a file
//! that is not an ODCS contract at all came back green. The fixture
//! `sniffed-odcl-specification-key.yaml` in `conform-odcs` is that document,
//! kept as a permanent record.
//!
//! What this module does is not that. It decides **which adapter to ask**, and
//! then the adapter answers for itself, under its own standard, with its own
//! codes. If the sniff routes a document to ODCS and the document is not an
//! ODCS contract, ODCS says so — loudly — rather than the routing quietly
//! choosing a standard the document happens to satisfy. And a file nothing
//! here recognises is reported under [`codes::UNRECOGNISED_DOCUMENT`], never
//! skipped: a file silently ignored is a file everybody believes was checked.
//!
//! Routing a `dataContractSpecification` document to `conform-lexicon` is the
//! *opposite* of that defect rather than a repetition of it. The defect was
//! recognising ODCL and then reporting the answer as ODCS; here the document
//! is recognised as ODCL, handed to the ODCL adapter, and reported under ODCL
//! codes against the ODCL schema — and if it turns out not to be an ODCL
//! document, `conform-lexicon` fails it, which is what the fixture
//! `faulty-odcs-document.yaml` in that crate records.
//!
//! `--spec` overrides the sniff entirely, which is the escape hatch for a
//! document whose `kind` is wrong — exactly the case where you want a verdict
//! rather than a routing decision.

use std::fs;
use std::path::{Path, PathBuf};

use conform_core::{Diagnostic, Location};

use crate::codes;
use crate::model::{FileStandard, Standard};

/// File extensions this module will try to read as a single-document
/// specification. ODCS and ODPS documents are YAML in practice and JSON
/// occasionally; both adapters parse both.
const DOCUMENT_EXTENSIONS: [&str; 3] = ["yaml", "yml", "json"];

/// The file whose presence in a directory makes it an OKF bundle root.
///
/// From the specification's own layout, and from upstream's published corpus:
/// every bundle in `conform-okf`'s fixtures has one, and `okf_core::Bundle`
/// reads its frontmatter for the declared `okf_version`.
const BUNDLE_MARKER: &str = "index.md";

/// How deep a directory walk will go before giving up.
///
/// A bound rather than a guess: a symlink loop is a real thing to walk into,
/// and a stack overflow is a worse way to report one than stopping.
const MAX_DEPTH: usize = 32;

/// Something to check, and what to check it as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A single document, already read, to be handed to one adapter.
    Document {
        /// What the document is called in diagnostics — the path as given.
        id: String,
        /// Its text, read once here so the adapter does not read it again.
        text: String,
        /// The adapter to ask. Never OKF: an OKF bundle is a directory, and
        /// the type says so.
        standard: FileStandard,
    },
    /// A directory that is an OKF bundle root.
    Bundle {
        /// What the bundle is called in diagnostics.
        id: String,
        /// Its root directory.
        path: PathBuf,
    },
}

impl Target {
    /// What this target is called in diagnostics.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Document { id, .. } | Self::Bundle { id, .. } => id,
        }
    }

    /// The standard this target will be checked against.
    #[must_use]
    pub const fn standard(&self) -> Standard {
        match self {
            Self::Document { standard, .. } => standard.standard(),
            Self::Bundle { .. } => Standard::Okf,
        }
    }
}

/// What a walk of the given paths found.
#[derive(Debug, Default)]
pub struct Discovery {
    /// Everything that will be checked, in a deterministic order.
    pub targets: Vec<Target>,
    /// Everything that will not be, and why. Always reported.
    pub problems: Vec<Diagnostic>,
}

/// Walk every path given on the command line.
///
/// `forced` is `--spec`: when it is set, sniffing is skipped and every file
/// found is handed to that adapter. When it is not, each file is routed by its
/// `kind` key and a file with no recognisable `kind` is reported rather than
/// skipped.
#[must_use]
pub fn walk(paths: &[PathBuf], forced: Option<Standard>) -> Discovery {
    let mut discovery = Discovery::default();
    for path in paths {
        visit(path, forced, 0, &mut discovery);
    }
    discovery
}

/// One path, at one depth.
fn visit(path: &Path, forced: Option<Standard>, depth: usize, found: &mut Discovery) {
    if depth > MAX_DEPTH {
        found.problems.push(
            Diagnostic::warning(
                codes::PATH_UNREADABLE,
                Location::document(display(path)),
                format!("stopped walking at {MAX_DEPTH} directories deep"),
            )
            .with_help("a symlink loop, or a tree deeper than anything this was built for"),
        );
        return;
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            found.problems.push(Diagnostic::error(
                codes::PATH_UNREADABLE,
                Location::document(display(path)),
                format!("cannot read this path: {error}"),
            ));
            return;
        }
    };

    if metadata.is_dir() {
        visit_directory(path, forced, depth, found);
    } else {
        visit_file(path, forced, found);
    }
}

/// A directory: an OKF bundle root, or a tree to walk.
fn visit_directory(path: &Path, forced: Option<Standard>, depth: usize, found: &mut Discovery) {
    // A bundle root is a leaf as far as the walk is concerned: its markdown
    // belongs to the bundle, and `okf_core` reads the tree itself. Only skipped
    // when `--spec` asks for one of the single-document standards, where the
    // caller has said in as many words that bundles are not what they meant.
    let is_bundle_root = path.join(BUNDLE_MARKER).is_file();
    let wants_bundles = forced.is_none_or(|standard| standard == Standard::Okf);
    if is_bundle_root && wants_bundles {
        found.targets.push(Target::Bundle {
            id: display(path),
            path: path.to_path_buf(),
        });
        return;
    }

    let entries = match read_dir_sorted(path) {
        Ok(entries) => entries,
        Err(error) => {
            found.problems.push(Diagnostic::error(
                codes::PATH_UNREADABLE,
                Location::document(display(path)),
                format!("cannot list this directory: {error}"),
            ));
            return;
        }
    };

    for entry in entries {
        if is_ignored(&entry) {
            continue;
        }
        // A file inside a directory is only *checked* if it looks like one of
        // ours. Reporting every README in a repository as unrecognised would
        // make the diagnostic worthless; a file named on the command line is a
        // different matter, and is reported.
        if entry.is_dir() || has_document_extension(&entry) {
            visit(&entry, forced, depth + 1, found);
        }
    }
}

/// A file named on the command line, or found in a walk.
fn visit_file(path: &Path, forced: Option<Standard>, found: &mut Discovery) {
    let id = display(path);

    if forced == Some(Standard::Okf) {
        found.problems.push(
            Diagnostic::warning(
                codes::UNRECOGNISED_DOCUMENT,
                Location::document(id),
                "`--spec okf` was asked for, and this is a file".to_owned(),
            )
            .with_help("an OKF bundle is a directory with an `index.md` at its root"),
        );
        return;
    }

    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            found.problems.push(Diagnostic::error(
                codes::PATH_UNREADABLE,
                Location::document(id),
                format!("cannot read this file: {error}"),
            ));
            return;
        }
    };

    if let Some(standard) = forced.and_then(Standard::as_file).or_else(|| sniff(&text)) {
        found.targets.push(Target::Document { id, text, standard });
        return;
    }

    found.problems.push(
        Diagnostic::warning(
            codes::UNRECOGNISED_DOCUMENT,
            Location::document(id),
            match kind_in(&text) {
                Some(kind) => format!("`kind: {kind}` names no standard this binary validates"),
                None => format!(
                    "no `kind` key and no `{LEXICON_ROOT_KEY}` key, so nothing here can tell \
                     which standard this is"
                ),
            },
        )
        .with_help(
            "ODCS documents carry `kind: DataContract`, ODPS documents `kind: DataProduct`, and \
             ODCL documents a root `dataContractSpecification`; pass `--spec <id>` to check this \
             document as one of them anyway",
        ),
    );
}

/// The root key every ODCL document is required to carry.
///
/// Not a heuristic: the vendored schema's `required` is
/// `["dataContractSpecification", "id", "info"]` and the key is an enum of the
/// specification versions, so a document without it is not an ODCL document
/// and a document with it declares which standard it is written in. It is that
/// standard's `kind`, spelled the way that standard spells it.
const LEXICON_ROOT_KEY: &str = "dataContractSpecification";

/// Which standard a document declares itself to be, if it declares one.
///
/// Reads the discriminator each standard defines for itself — `kind` for the
/// two Bitol standards, the required root [`LEXICON_ROOT_KEY`] for ODCL — and
/// draws one conclusion. Everything else about the document is the adapter's
/// business.
///
/// `kind` is consulted first, and that ordering is deliberate: a document that
/// says `kind: DataContract` has named itself an ODCS contract, and must go to
/// ODCS to be told what is wrong with it even if it also carries an ODCL key.
#[must_use]
pub fn sniff(text: &str) -> Option<FileStandard> {
    let value: serde_norway::Value = serde_norway::from_str(text).ok()?;

    if let Some(kind) = kind_of(&value) {
        return match kind.as_str() {
            "DataContract" => Some(FileStandard::Odcs),
            "DataProduct" => Some(FileStandard::Odps),
            _ => None,
        };
    }

    value.get(LEXICON_ROOT_KEY).map(|_| FileStandard::Odcl)
}

/// The document's `kind`, as written.
fn kind_of(value: &serde_norway::Value) -> Option<String> {
    value.get("kind")?.as_str().map(str::to_owned)
}

/// The document's `kind`, read from its text.
///
/// Parsed with `serde_norway` — the same parser every adapter here uses — so a
/// document that sniffs one way cannot fail to parse there for a reason this
/// module invented.
fn kind_in(text: &str) -> Option<String> {
    let value: serde_norway::Value = serde_norway::from_str(text).ok()?;
    kind_of(&value)
}

/// A directory's entries, sorted, so two runs over one tree report in one
/// order.
fn read_dir_sorted(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    Ok(entries)
}

/// Whether a walk should step over this entry.
///
/// Dot-directories and build output. Not a general ignore mechanism — a path
/// named explicitly on the command line is always visited, whatever it is
/// called.
fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.') || name == "target")
}

/// Whether this file's extension is one a single-document adapter reads.
fn has_document_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            DOCUMENT_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

/// A path as it will be named in diagnostics.
fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_routes_to_the_adapter_that_owns_it() {
        assert_eq!(
            sniff("apiVersion: v3.1.0\nkind: DataContract\n"),
            Some(FileStandard::Odcs)
        );
        assert_eq!(
            sniff("apiVersion: v1.0.0\nkind: DataProduct\n"),
            Some(FileStandard::Odps)
        );
    }

    #[test]
    fn an_odcl_document_is_routed_by_the_root_key_its_own_schema_requires() {
        // The `validate_odcs_internal` defect was recognising this document
        // and then answering as ODCS. Recognising it and answering as ODCL is
        // the repair, not a repetition: the verdict comes from the ODCL
        // schema, under ODCL codes.
        assert_eq!(
            sniff("dataContractSpecification: 1.1.0\nid: urn:x\n"),
            Some(FileStandard::Odcl)
        );
        // Every version the schema's enum holds, not just the current one.
        assert_eq!(
            sniff("dataContractSpecification: 0.9.0\n"),
            Some(FileStandard::Odcl)
        );
        // And a value the enum does *not* hold still routes here, because
        // rejecting it is the adapter's job and it has an `ODCL103` for it.
        assert_eq!(
            sniff("dataContractSpecification: 2.0.0\n"),
            Some(FileStandard::Odcl)
        );
    }

    #[test]
    fn a_kind_that_names_a_bitol_standard_wins_over_the_lexicon_key() {
        // A document that calls itself an ODCS contract is checked as one,
        // whatever else it carries. Otherwise a stray key would silently
        // redirect a document away from the standard it declares.
        assert_eq!(
            sniff("kind: DataContract\ndataContractSpecification: 1.2.1\n"),
            Some(FileStandard::Odcs)
        );
        // And a `kind` naming something else is not quietly re-routed either.
        assert_eq!(
            sniff("kind: Deployment\ndataContractSpecification: 1.2.1\n"),
            None
        );
    }

    #[test]
    fn a_document_that_names_no_standard_is_not_routed_to_a_guess() {
        assert_eq!(sniff("kind: Deployment\n"), None);
        assert_eq!(sniff("not: even: yaml: ["), None);
        assert_eq!(sniff(""), None);
        // A sequence at the root has no keys to read, so nothing here can say
        // what it is. `conform-lexicon` has a fixture for exactly this shape;
        // reaching that verdict needs `--spec odcl`.
        assert_eq!(sniff("- dataContractSpecification: 1.2.1\n"), None);
    }

    #[test]
    fn json_sniffs_too_because_both_adapters_read_it() {
        assert_eq!(
            sniff(r#"{"apiVersion": "v1.0.0", "kind": "DataProduct"}"#),
            Some(FileStandard::Odps)
        );
    }
}
