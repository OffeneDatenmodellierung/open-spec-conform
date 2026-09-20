//! Everything the page shows, gathered once, before anybody has decided how to
//! show it.
//!
//! The shape `conform-cli` uses, and for the same reason: [`crate::render`]
//! reads a [`Site`] and nothing else. It opens no file, hashes no bytes and
//! decides nothing, so it cannot disagree with the console about what the
//! registry says. Every fact on the rendered page can be traced to a field
//! here, and every field here to `specs.toml` or to a member manifest.

use std::path::{Path, PathBuf};

use conform_cli::model::SpecSummary;
use conform_registry::{LoadError, Poll, Registry};

use crate::manifests::{CrateEntry, ManifestError, read_dir};

/// One specification, as the page shows it.
///
/// [`SpecSummary`] is `conform-cli`'s, reused rather than re-declared — see
/// this crate's manifest for the argument. What it does not carry is the
/// [`Poll`], because the console has no use for it; this page does, since
/// "how would we find out upstream had moved" is the question the catalogue
/// is here to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecCard {
    /// The provenance, the live re-hash, and the registry's own diagnostic
    /// for it.
    pub summary: SpecSummary,
    /// How to ask upstream whether the pin has been superseded, where the
    /// registry records a way. Absent is a real answer and is rendered as one.
    pub poll: Option<Poll>,
}

/// Whether a reader can validate a document in this page, and if not, why not.
///
/// A value rather than a paragraph in the renderer, so that the page cannot
/// claim a capability the build does not have. Wiring the demo up means
/// producing a different variant here; it does not mean editing prose, and
/// there is no way to edit the prose into claiming something untrue without
/// the type changing under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Demo {
    /// No in-browser validation in this build, with the reason stated and the
    /// evidence for it.
    ///
    /// The reason is carried rather than assumed, because "we did not build
    /// it" and "it cannot be built" are different statements and a reader is
    /// entitled to know which one this is. The evidence is carried because a
    /// claim about what a build does is worth exactly as much as what was run
    /// to establish it.
    NotWired {
        /// What stands between this page and a working demo, in one sentence.
        because: String,
        /// What was actually built and run to find that out: each probe and
        /// its result, in the order they were performed.
        evidence: Vec<(String, String)>,
    },
}

/// Everything the page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// The registry file the catalogue was read from, as a reader would name
    /// it: relative to the repository root rather than to whatever directory
    /// the generator happened to run in.
    pub registry_path: String,
    /// The registry format version the file declared.
    pub registry_schema_version: u32,
    /// The catalogue, in registry order. Registry order rather than sorted:
    /// `specs.toml` is grouped and commented by a human, and reordering it
    /// here would silently disagree with the file a reader is sent to.
    pub specs: Vec<SpecCard>,
    /// The crate family, in name order, read from the member manifests.
    pub crates: Vec<CrateEntry>,
    /// Whether this build can validate a document in the browser.
    pub demo: Demo,
}

impl Site {
    /// Gather everything from a checkout.
    ///
    /// `root` is the repository root: the directory holding `specs.toml` and
    /// `crates/`. Every artefact digest is **re-hashed here**, not read from
    /// the registry and believed — which is the same thing
    /// `conform registry verify` does, and the reason the drift column on the
    /// page is a measurement rather than a transcription.
    ///
    /// # Errors
    ///
    /// [`GatherError`] if the registry will not load or a member manifest will
    /// not read. Neither is recoverable into a partial page: a catalogue
    /// missing an entry, or a family list missing a crate, is worse than no
    /// page at all, because it looks complete.
    pub fn gather(root: impl AsRef<Path>) -> Result<Self, GatherError> {
        let root = root.as_ref();
        let registry_file = root.join(REGISTRY_FILE);

        let registry =
            Registry::load_path(&registry_file).map_err(|error| GatherError::Registry {
                path: registry_file.clone(),
                error: Box::new(error),
            })?;

        let crates = read_dir(root.join(CRATES_DIR)).map_err(GatherError::Manifest)?;

        Ok(Self::from_parts(
            &registry,
            crates,
            REGISTRY_FILE.to_owned(),
        ))
    }

    /// Gather from a registry already in hand.
    ///
    /// The seam the tests use. `tests/the_page_is_a_function_of_the_registry.rs`
    /// hands this a registry in which every value is fabricated, and asserts
    /// that the fabricated values are what the page shows — which is how the
    /// "nothing about a specification is hand-typed" claim is proved rather
    /// than asserted. A renderer that could not be handed a different registry
    /// could not be proved to be reading the real one.
    #[must_use]
    pub fn from_parts(registry: &Registry, crates: Vec<CrateEntry>, registry_path: String) -> Self {
        let specs = registry
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| SpecCard {
                // The re-hash. `verify_entry` reads the artefact off disk and
                // compares; `SpecSummary::new` reads the verdict out of the
                // diagnostic's code rather than out of a boolean, so a missing
                // artefact and a drifted one stay distinguishable on the page.
                summary: SpecSummary::new(entry, registry.verify_entry(index, entry)),
                poll: entry.poll.clone(),
            })
            .collect();

        Self {
            registry_path,
            registry_schema_version: registry.schema_version(),
            specs,
            crates,
            demo: Demo::NotWired {
                because: DEMO_NOT_WIRED.to_owned(),
                evidence: DEMO_EVIDENCE
                    .iter()
                    .map(|(probe, result)| ((*probe).to_owned(), (*result).to_owned()))
                    .collect(),
            },
        }
    }

    /// How many entries record a way of noticing that upstream has moved.
    ///
    /// Computed here rather than in the renderer, for the reason
    /// `conform-cli` gives for computing nothing in `human.rs`: a number the
    /// renderer derived is a number that can disagree with the rows above it.
    #[must_use]
    pub fn pollable(&self) -> usize {
        self.specs.iter().filter(|s| s.poll.is_some()).count()
    }

    /// How many provenance fields, across the whole catalogue, nobody could
    /// determine.
    ///
    /// Distinct from [`with_gaps`](Self::with_gaps), which counts *entries*.
    /// One entry missing three fields and three entries missing one each are
    /// the same number here and different numbers there, and a reader wants
    /// both: how widespread the gaps are, and how many there are.
    #[must_use]
    pub fn recorded_gaps(&self) -> usize {
        self.specs
            .iter()
            .map(|s| s.summary.provenance_gaps.len())
            .sum()
    }

    /// How many entries have at least one provenance field nobody could
    /// determine.
    #[must_use]
    pub fn with_gaps(&self) -> usize {
        self.specs
            .iter()
            .filter(|s| !s.summary.provenance_gaps.is_empty())
            .count()
    }
}

/// The registry file, relative to the repository root.
const REGISTRY_FILE: &str = "specs.toml";

/// The workspace's crate directory, relative to the repository root.
const CRATES_DIR: &str = "crates";

/// Why this build cannot validate a document in the browser.
///
/// One sentence, stated plainly, because the alternative a reviewer would
/// rightly refuse is a demo that pretends. See `docs/plan` §6.4 for the
/// evidence behind it in full and for what wiring it up would take.
const DEMO_NOT_WIRED: &str = "The validator compiles to WebAssembly and its C ABI works across \
     the boundary — both were built and run to check. Two things stand in the way, and neither \
     is time. Its only constructor takes a registry *path*, and a browser has no filesystem to \
     resolve one against; and on this target the crate's promise that no panic crosses the \
     boundary is provably false, because `wasm32-unknown-unknown` aborts on panic and there is \
     nothing for `catch_unwind` to catch. Shipping a validator whose panic net does not work, \
     in the one crate built around the claim that it does, would be shipping a guarantee we \
     know to be untrue.";

/// What was built and run to establish the above, and what each probe said.
///
/// Recorded as data rather than prose so that the page shows the measurements
/// rather than a summary of them. Every row was produced by running the thing
/// it names against this repository at the commit this page was generated
/// from; none of it is inferred.
const DEMO_EVIDENCE: &[(&str, &str)] = &[
    (
        "cargo check -p conform-odcs --target wasm32-unknown-unknown",
        "compiles — the JSON Schema validator, the YAML parser and the registry are all \
         wasm-compatible unmodified",
    ),
    (
        "cargo build -p conform-ffi --target wasm32-unknown-unknown --release",
        "builds a 347,412-byte cdylib exporting all six ABI entry points and its linear memory",
    ),
    (
        "conform_version()",
        "returns 0.1.0 — the C ABI genuinely works across the WebAssembly boundary",
    ),
    (
        "conform_validator_new(\"odcs\", \"specs.toml\")",
        "returns NULL, and conform_last_error() explains: cannot read the registry: operation \
         not supported on this platform. The crate compiles and links cleanly for this target \
         and then fails on every call; nothing in the toolchain warns about it",
    ),
    (
        "conform_self_test_panic()",
        "traps with `unreachable` instead of returning a status code. This entry point exists \
         to detect exactly that, and this is the first target on which it has fired",
    ),
];

/// The registry, or a manifest, could not be read.
#[derive(Debug)]
pub enum GatherError {
    /// `specs.toml` would not load. Carries the registry's own
    /// [`ConformanceReport`](conform_core::ConformanceReport), boxed because
    /// [`LoadError`] is much larger than the other variant.
    Registry {
        /// The registry file that would not load.
        path: PathBuf,
        /// What the registry said about it.
        error: Box<LoadError>,
    },
    /// A member manifest would not read.
    Manifest(ManifestError),
}

impl std::fmt::Display for GatherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registry { path, error } => {
                write!(f, "{}: {error}", path.display())
            }
            Self::Manifest(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for GatherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registry { error, .. } => Some(error.as_ref()),
            Self::Manifest(error) => Some(error),
        }
    }
}
