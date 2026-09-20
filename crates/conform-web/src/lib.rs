//! The Open Spec Conform site: one page, generated from the registry.
//!
//! # What the page is for
//!
//! Three jobs, in the order a reader meets them:
//!
//! 1. **Describe the tool** — what this project is, and the defect it was
//!    built around.
//! 2. **Show the specification catalogue with its canonical upstream links** —
//!    for every vendored artefact, where it came from, the immutable revision
//!    it was pinned at, and the digest of the bytes in the tree. This is the
//!    page's centre of gravity rather than a footnote, because the reason to
//!    record an upstream link is to be able to notice when upstream moves.
//! 3. **List the crate family** — each independently versioned, each pinnable
//!    without the rest.
//!
//! # Nothing about a specification is written in this crate
//!
//! No URL, no name, no identifier, no version and no digest appears as a
//! literal anywhere in this source. Every one is read from `specs.toml`
//! through [`conform_registry`], and every crate version is read from that
//! crate's own manifest. That is the whole point: a hand-typed upstream link
//! is exactly the drift this project exists to prevent, and a site that
//! transcribed the registry would be a second copy of it to keep in step.
//!
//! Two tests hold the claim up, and neither can pass vacuously:
//!
//! - `tests/the_page_is_a_function_of_the_registry.rs` renders the page from a
//!   registry of **fabricated** values and asserts that the fabricated values
//!   are what comes out and the real ones do not. Anything hand-typed survives
//!   the substitution and is caught.
//! - `tests/no_spec_facts_are_written_in_the_source.rs` scans this crate's own
//!   source for the real registry's URLs, digests and pins — and proves the
//!   scanner works by running it against a string that contains one.
//!
//! # Absence is rendered as absence
//!
//! `specs.toml` deliberately omits `licence` on three of its five entries and
//! `homepage` on three, and says in each entry's notes what was looked at and
//! what it did not say. The page shows every such gap as `(not recorded)` —
//! the console's words, so a reader meets one vocabulary and not two — and
//! never as a blank, an empty string or a plausible default.
//!
//! # Escaping
//!
//! Registry notes and diagnostic messages quote third-party content verbatim,
//! and the adapters that produce them deliberately do not sanitise it. The
//! console neutralises it for a terminal; this crate neutralises it for a
//! browser. See [`escape`] for what that means and why the two are not the
//! same function.
//!
//! # Example
//!
//! ```no_run
//! let site = conform_web::Site::gather(".")?;
//! let html = conform_web::render::page(&site);
//! assert!(html.starts_with("<!DOCTYPE html>"));
//! # Ok::<(), conform_web::GatherError>(())
//! ```

pub mod escape;
pub mod manifests;
pub mod render;
pub mod site;

pub use manifests::{CrateEntry, ManifestError};
pub use site::{Demo, GatherError, Site, SpecCard};

use std::fs;
use std::io;
use std::path::Path;

/// The file the page is written to inside the output directory.
///
/// `index.html`, so that the output directory is servable as-is by anything
/// that serves a directory, and openable as-is by a browser given the
/// directory.
pub const PAGE_FILE: &str = "index.html";

/// Generate the site from a checkout into an output directory.
///
/// `root` is the repository root — the directory holding `specs.toml` and
/// `crates/`. `out` is created if it does not exist.
///
/// Returns the path written.
///
/// # Errors
///
/// [`BuildError::Gather`] if the registry or a manifest will not read;
/// [`BuildError::Write`] if the output cannot be written. A page is never
/// written from partial input: a catalogue missing an entry looks complete,
/// which is the one failure a provenance page must not have.
pub fn build(
    root: impl AsRef<Path>,
    out: impl AsRef<Path>,
) -> Result<std::path::PathBuf, BuildError> {
    let site = Site::gather(root).map_err(BuildError::Gather)?;
    let html = render::page(&site);

    let out = out.as_ref();
    fs::create_dir_all(out).map_err(BuildError::Write)?;
    let page = out.join(PAGE_FILE);
    fs::write(&page, html).map_err(BuildError::Write)?;
    Ok(page)
}

/// The site could not be generated.
#[derive(Debug)]
pub enum BuildError {
    /// The registry or a member manifest would not read.
    Gather(GatherError),
    /// The output could not be written.
    Write(io::Error),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gather(error) => write!(f, "{error}"),
            Self::Write(error) => write!(f, "cannot write the site: {error}"),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Gather(error) => Some(error),
            Self::Write(error) => Some(error),
        }
    }
}
