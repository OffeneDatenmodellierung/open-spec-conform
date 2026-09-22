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
///
/// Which variant this is, is a **measurement**: [`Demo::look_for`] stats the
/// artefacts and reports what it found. A build that could not produce a
/// module — no `wasm32-unknown-unknown` target, no `wasm-bindgen` CLI, a
/// compiler error — therefore produces a page that says so, rather than a page
/// with a validation box that throws on first use. That failure mode is the
/// one this type exists to make impossible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Demo {
    /// A WebAssembly module was found next to the page, and the page loads it.
    ///
    /// Carries what was measured rather than a boolean, because "there is a
    /// demo" is a claim and the bytes behind it are the evidence — a reader
    /// about to download three megabytes over a phone connection is entitled
    /// to know that before the page starts fetching.
    Wired(Module),

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

impl Demo {
    /// Look for a `wasm-bindgen` module in `directory`, and report what is
    /// there.
    ///
    /// `href_prefix` is how the page will reach it — a path relative to the
    /// page, not to the generator's working directory, because those are
    /// different things and only one of them means anything in a browser.
    ///
    /// The hrefs come back **`./`-prefixed**, and that is not cosmetic: a
    /// browser resolving an `import()` treats a bare specifier as a name for a
    /// module map to look up, not as a path, and refuses it. The first build
    /// of this produced `wasm/conform_ffi.js`, and Chrome answered
    /// `Failed to resolve module specifier`. What made that a two-minute
    /// problem rather than a shipped one is that the page reports a load
    /// failure instead of swallowing it — but the fix belongs here, where the
    /// specifier is made.
    ///
    /// Both files are required and both must be non-empty. A zero-byte module
    /// is the signature of an interrupted build, and a page that loaded one
    /// would fail in the browser rather than at generation time, which is the
    /// wrong end.
    #[must_use]
    pub fn look_for(directory: &Path, href_prefix: &str) -> Self {
        let script = directory.join(MODULE_SCRIPT);
        let wasm = directory.join(MODULE_WASM);

        let mut evidence = Vec::new();
        let script_bytes = measure(
            &script,
            &format!("{href_prefix}/{MODULE_SCRIPT}"),
            &mut evidence,
        );
        let wasm_bytes = measure(
            &wasm,
            &format!("{href_prefix}/{MODULE_WASM}"),
            &mut evidence,
        );

        match (script_bytes, wasm_bytes) {
            (Some(script_bytes), Some(wasm_bytes)) => Self::Wired(Module {
                script_href: format!("./{href_prefix}/{MODULE_SCRIPT}"),
                script_bytes,
                wasm_href: format!("./{href_prefix}/{MODULE_WASM}"),
                wasm_bytes,
            }),
            _ => Self::NotWired {
                because: DEMO_NOT_BUILT.to_owned(),
                evidence,
            },
        }
    }

    /// The state of a page generated by something that never looked.
    ///
    /// Distinct from [`look_for`](Self::look_for) finding nothing, and the
    /// distinction is the point: "we looked and there was no module" and "we
    /// were never told where to look" are different facts, and a page that
    /// blurred them would be claiming a measurement it did not take.
    #[must_use]
    pub fn not_looked_for() -> Self {
        Self::NotWired {
            because: DEMO_NOT_LOOKED_FOR.to_owned(),
            evidence: vec![(
                "conform-web --wasm <dir>".to_owned(),
                "not given, so nothing was looked for and nothing can be reported".to_owned(),
            )],
        }
    }
}

/// The `wasm-bindgen` artefacts the page loads, as measured on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The ES module, as a specifier the page can `import()`: relative to the
    /// page and `./`-prefixed. See [`Demo::look_for`] for why the prefix is
    /// load-bearing rather than tidy.
    pub script_href: String,
    /// Its size in bytes.
    pub script_bytes: u64,
    /// The WebAssembly binary, relative to the page.
    pub wasm_href: String,
    /// Its size in bytes.
    pub wasm_bytes: u64,
}

/// Stat one artefact, recording what was found either way.
///
/// The evidence row is written whether or not the file is there, so a page
/// that says "not wired" says which file was missing rather than leaving a
/// reader to guess between two.
///
/// `name` is what the row says, and it is the path **relative to the page**
/// rather than the one that was stat'd. The two differ whenever a build
/// passes an absolute `--wasm`, and the absolute one is the generator's
/// working directory printed on a public web page: it tells a reader nothing
/// they can act on and tells everybody else the layout of a build machine.
fn measure(path: &Path, name: &str, evidence: &mut Vec<(String, String)>) -> Option<u64> {
    let name = name.to_owned();
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.len() > 0 => {
            evidence.push((name, format!("{} bytes", metadata.len())));
            Some(metadata.len())
        }
        Ok(_) => {
            evidence.push((
                name,
                "present and empty, which is an interrupted build".to_owned(),
            ));
            None
        }
        Err(error) => {
            evidence.push((name, error.to_string()));
            None
        }
    }
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
            .flat_map(|(entry_index, entry)| {
                let default_vi = entry.default_version_index();
                entry
                    .versions
                    .iter()
                    .enumerate()
                    .map(move |(version_index, version)| SpecCard {
                        summary: SpecSummary::new(
                            entry,
                            version,
                            version_index == default_vi,
                            registry.verify_version(entry_index, version_index, version),
                        ),
                        poll: version.poll.clone(),
                    })
            })
            .collect();

        Self {
            registry_path,
            registry_schema_version: registry.schema_version(),
            specs,
            crates,
            demo: Demo::not_looked_for(),
        }
    }

    /// Replace the demo state with one somebody measured.
    ///
    /// The seam [`crate::build`] uses: [`gather`](Self::gather) knows where
    /// the *repository* is and has no idea where the page is being written,
    /// and the module sits next to the page. Rather than teach the gatherer
    /// about output directories, the measurement is taken where the output
    /// directory is known and handed in here.
    #[must_use]
    pub fn with_demo(mut self, demo: Demo) -> Self {
        self.demo = demo;
        self
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

/// The `wasm-bindgen` glue, as that tool names it for this crate.
///
/// Transcribed, because the name is `wasm-bindgen`'s own function of the crate
/// name and nothing here can read it out of anywhere. It is the safe direction
/// to be wrong in: a name that stopped matching makes [`Demo::look_for`] find
/// nothing and the page say so, rather than emit a script tag pointing at a
/// file that is not there.
const MODULE_SCRIPT: &str = "conform_ffi.js";

/// The WebAssembly binary the glue loads. See [`MODULE_SCRIPT`].
const MODULE_WASM: &str = "conform_ffi_bg.wasm";

/// Why a page generated without a module says it has no demo.
///
/// Note what this sentence does *not* say. Until this build existed, the
/// answer here was that an in-browser demo could not honestly be shipped at
/// all: the only constructor took a registry path and a browser has no
/// filesystem, and on `wasm32-unknown-unknown` the FFI crate's promise that no
/// panic crosses its boundary is false, because that target aborts on panic
/// and leaves nothing for `catch_unwind` to catch. Both are now answered —
/// the registry and the vendored schemas are compiled into the module and the
/// digest is re-checked there, and the binding states the panic contract
/// instead of claiming a net it cannot have. So what is left is an ordinary
/// build failure, and it is reported as one.
const DEMO_NOT_BUILT: &str = "This build produced no WebAssembly module, so there is nothing for \
     the page to load. That is a build result rather than a limitation: the binding exists, it \
     embeds the registry and the vendored schemas so it needs no filesystem, and it states \
     plainly that a panic is fatal to the instance rather than claiming a net that \
     `wasm32-unknown-unknown` cannot provide. Running `tools/wasm/build.sh` before this \
     generator is what puts the module next to the page.";

/// Why a page generated by something that never looked says it has no demo.
const DEMO_NOT_LOOKED_FOR: &str = "This page was generated without being told where to look for \
     a WebAssembly module, so it cannot say whether one was built. It does not follow that \
     there is no demo; it follows that this page has no measurement to report, and a page with \
     no measurement must not make a claim.";

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
