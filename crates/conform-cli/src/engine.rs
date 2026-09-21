//! Doing the work, once.
//!
//! Everything a renderer shows comes from here. This module loads the
//! registry, decides which specifications are in scope, asks the adapters
//! about the documents it was given, and hands back a [`Run`]. It renders
//! nothing and it prints nothing, which is what lets the human output, the
//! `--json` envelope and the TUI be three views of one set of facts rather
//! than three implementations of one idea.
//!
//! # Provenance first, always
//!
//! Every adapter here is built through [`Registry`], never from a hard-coded
//! path, so the vendored schema is re-hashed against the digest `specs.toml`
//! records for it *before* any verdict is issued against it. A schema that has
//! drifted produces a refusal, and every document that would have been checked
//! against it is recorded as **not checked** under
//! [`codes::NOT_CHECKED`](crate::codes::NOT_CHECKED). A document quietly
//! skipped is a document everybody believes passed.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use conform_core::{ConformanceReport, Diagnostic, GatePolicy, Location, Validator};
use conform_lexicon::LexiconValidator;
use conform_odcs::OdcsValidator;
use conform_odps::OdpsValidator;
use conform_registry::Registry;

use crate::codes;
use crate::discover::{self, Target};
use crate::model::{
    COMMAND_LINE, Command, DocumentOutcome, FileStandard, Run, SpecSummary, Standard,
};

/// What one invocation asked for.
///
/// The parsed, validated form of the command line: [`crate::cli`] turns
/// arguments into this, and this module turns it into a [`Run`]. Splitting
/// them keeps the engine testable without a command line, and keeps `clap` out
/// of every signature below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Which subcommand.
    pub command: Command,
    /// The paths to check. Empty for the registry commands.
    pub paths: Vec<PathBuf>,
    /// `--spec`: restrict everything to one registry entry.
    pub spec: Option<String>,
    /// `--gate` / `--check`: what, if anything, fails the run.
    pub policy: GatePolicy,
    /// `--registry`: where `specs.toml` is, when it is not to be searched for.
    pub registry: Option<PathBuf>,
}

/// Do the work.
///
/// Never fails: a registry that will not load, a `--spec` naming nothing and a
/// path that is not there are all *findings*, reported through the same
/// [`ConformanceReport`] as a malformed contract. What they change is
/// [`Run::unusable`], and through it the exit code — because "I could not do
/// the work" must not exit the same way as "I did the work and it passed".
#[must_use]
pub fn run(request: &Request) -> Run {
    let registry_path = match locate_registry(request.registry.as_deref()) {
        Ok(path) => path,
        Err(diagnostic) => return unusable(request, "specs.toml", diagnostic),
    };
    let display_path = registry_path.display().to_string();

    let registry = match Registry::load_path(&registry_path) {
        Ok(registry) => registry,
        Err(error) => {
            return Run {
                command: request.command,
                policy: request.policy,
                registry_path: display_path.clone(),
                specs: Vec::new(),
                documents: vec![DocumentOutcome::new(
                    display_path,
                    None,
                    error.into_report(),
                )],
                unusable: true,
            };
        }
    };

    let mut run = Run {
        command: request.command,
        policy: request.policy,
        registry_path: display_path.clone(),
        specs: summarise(&registry, request.spec.as_deref()),
        documents: Vec::new(),
        unusable: false,
    };

    if let Some(id) = &request.spec
        && run.specs.is_empty()
    {
        run.documents.push(DocumentOutcome::new(
            display_path,
            None,
            single(
                Diagnostic::error(
                    codes::UNKNOWN_SPEC,
                    Location::document(run.registry_path.as_str()),
                    format!("`--spec {id}` names no entry in this registry"),
                )
                .with_help(known_ids(&registry)),
            ),
        ));
        run.unusable = true;
        return run;
    }

    match request.command {
        Command::RegistryList => list(&registry, &mut run),
        Command::RegistryVerify => verify(&mut run),
        Command::Validate => validate(&registry, request, &mut run),
    }
    run
}

/// `registry list`: the catalogue, plus what the registry's own rules say
/// about it.
///
/// The rules are included rather than left to `verify` because they answer a
/// different question — *does this record what a provenance record must
/// record* — and a catalogue printed without them would show a gap as an
/// absence rather than as the finding it is.
fn list(registry: &Registry, run: &mut Run) {
    let mut report = registry.validate();
    if let Some(id) = run.specs.first().map(|spec| spec.id.clone())
        && run.specs.len() == 1
    {
        // `--spec` filters the findings the same way it filters the catalogue,
        // by the pointer the registry writes into every one of them.
        let index = registry.entries().iter().position(|entry| entry.id == id);
        if let Some(index) = index {
            report = report
                .into_iter()
                .filter(|diagnostic| mentions_entry(diagnostic, index))
                .collect();
        }
    }
    run.documents.push(DocumentOutcome::new(
        run.registry_path.clone(),
        None,
        report,
    ));
}

/// Whether a registry diagnostic is about the entry at this index.
///
/// Read from the `/spec/<index>/<field>` pointer the registry writes, which is
/// the only part of a diagnostic that says which entry it came from.
fn mentions_entry(diagnostic: &Diagnostic, index: usize) -> bool {
    diagnostic
        .location
        .pointer
        .as_ref()
        .is_some_and(|pointer| pointer.as_str().starts_with(&format!("/spec/{index}/")))
}

/// `registry verify`: the vendored bytes, re-hashed.
///
/// One outcome per entry, including the ones that pass. A silent success and a
/// skipped check are indistinguishable otherwise, which is the distinction
/// `conform-registry` was built after losing once already.
fn verify(run: &mut Run) {
    let documents: Vec<DocumentOutcome> = run
        .specs
        .iter()
        .map(|spec| {
            DocumentOutcome::new(
                spec.verify_diagnostic.location.document.as_str(),
                Some(spec.id.clone()),
                single(spec.verify_diagnostic.clone()),
            )
        })
        .collect();
    run.documents.extend(documents);
}

/// `validate`: the documents, against the standard each is written in.
fn validate(registry: &Registry, request: &Request, run: &mut Run) {
    if let Some(id) = &request.spec
        && Standard::from_spec_id(id).is_none()
    {
        run.documents.push(DocumentOutcome::new(
            run.registry_path.clone(),
            Some(id.clone()),
            single(
                Diagnostic::error(
                    codes::NO_ADAPTER_FOR_SPEC,
                    Location::document(run.registry_path.as_str()),
                    format!(
                        "`{id}` is in the registry and this binary has no validator for it, so \
                         it cannot check anything against it"
                    ),
                )
                .with_help(
                    "the catalogue is wider than the set of adapters; `conform registry list` \
                     shows both",
                ),
            ),
        ));
        run.unusable = true;
        return;
    }

    let forced = request.spec.as_deref().and_then(Standard::from_spec_id);
    let discovery = discover::walk(&request.paths, forced);

    if !discovery.problems.is_empty() {
        let mut report = ConformanceReport::new();
        report.extend(discovery.problems);
        run.documents
            .push(DocumentOutcome::new(COMMAND_LINE, None, report));
    }

    if discovery.targets.is_empty() {
        run.documents.push(DocumentOutcome::new(
            COMMAND_LINE,
            None,
            single(
                Diagnostic::error(
                    codes::NO_DOCUMENTS,
                    Location::document(COMMAND_LINE),
                    "the paths given hold no document this binary validates".to_owned(),
                )
                .with_help(
                    "nothing was checked, which is why this is an error and not silence — pass a \
                     path to a contract, a product, a data contract, or an OKF bundle directory",
                ),
            ),
        ));
        return;
    }

    let mut validators = Validators::new(registry);
    let outcomes: Vec<DocumentOutcome> = discovery
        .targets
        .iter()
        .map(|target| check(target, &mut validators))
        .collect();
    run.documents.extend(outcomes);
}

/// Check one target with the adapter that owns it.
fn check(target: &Target, validators: &mut Validators<'_>) -> DocumentOutcome {
    let standard = target.standard();
    let spec_id = Some(standard.spec_id().to_owned());

    match target {
        Target::Bundle { id, path } => DocumentOutcome::new(id, spec_id, bundle_report(path)),
        Target::Document { id, text, standard } => match validators.get(*standard) {
            Ok(validator) => DocumentOutcome::new(id, spec_id, validator.validate_text(id, text)),
            Err(report) => {
                DocumentOutcome::new(id, spec_id, not_checked(id, standard.standard(), report))
            }
        },
    }
}

/// An OKF bundle: conformance and hygiene, merged.
///
/// Both, because both are about this bundle and a reader wants one list. They
/// stay distinguishable by severity, which is the split that matters: nothing
/// hygiene raises is ever an error, so merging them cannot change whether the
/// bundle gates.
fn bundle_report(path: &Path) -> ConformanceReport {
    match conform_okf::load(path) {
        Ok(bundle) => {
            let mut report = conform_okf::validate_bundle(&bundle).into_report();
            report.merge(conform_okf::lint_bundle(&bundle).into_report());
            report
        }
        Err(error) => error.into_report(),
    }
}

/// The record of a document that was *not* checked, and why.
///
/// The adapter's own refusal is kept verbatim underneath, so the reader gets
/// the `ODCS9xx`/`REG0xx` code that explains the provenance failure rather
/// than a paraphrase of it.
fn not_checked(id: &str, standard: Standard, cause: &ConformanceReport) -> ConformanceReport {
    let mut report = single(
        Diagnostic::error(
            codes::NOT_CHECKED,
            Location::document(id),
            format!(
                "not checked: the `{}` validator could not be built",
                standard.spec_id()
            ),
        )
        .with_help(
            "a document nobody checked must not look like a document that passed — fix the \
             schema's provenance, do not bypass it",
        ),
    );
    report.extend(cause.iter().cloned());
    report
}

/// The adapters, built at most once each and only if a document needs one.
///
/// Building one compiles a JSON Schema and re-hashes the vendored bytes, so
/// building all of them to check a single file would be waste; building one
/// per document would be worse. A failure is cached alongside a success,
/// because a schema that failed its provenance check does not pass on the
/// second document either.
struct Validators<'r> {
    registry: &'r Registry,
    odcs: Option<Result<OdcsValidator, ConformanceReport>>,
    odps: Option<Result<OdpsValidator, ConformanceReport>>,
    odcl: Option<Result<LexiconValidator, ConformanceReport>>,
}

impl<'r> Validators<'r> {
    const fn new(registry: &'r Registry) -> Self {
        Self {
            registry,
            odcs: None,
            odps: None,
            odcl: None,
        }
    }

    /// The adapter for this standard, or the report explaining why there is
    /// not one.
    ///
    /// OKF cannot reach here, and the parameter type is why: an OKF bundle is
    /// checked by `conform_okf`'s free functions, which take the bundle rather
    /// than a constructed validator.
    fn get(&mut self, standard: FileStandard) -> Result<&dyn TextValidator, &ConformanceReport> {
        match standard {
            FileStandard::Odcs => {
                let slot = self.odcs.get_or_insert_with(|| {
                    OdcsValidator::from_registry(self.registry)
                        .map_err(conform_odcs::SchemaError::into_report)
                });
                match slot {
                    Ok(validator) => Ok(validator),
                    Err(report) => Err(report),
                }
            }
            FileStandard::Odps => {
                let slot = self.odps.get_or_insert_with(|| {
                    OdpsValidator::from_registry(self.registry)
                        .map_err(conform_odps::SchemaError::into_report)
                });
                match slot {
                    Ok(validator) => Ok(validator),
                    Err(report) => Err(report),
                }
            }
            FileStandard::Odcl => {
                let slot = self.odcl.get_or_insert_with(|| {
                    LexiconValidator::from_registry(self.registry)
                        .map_err(conform_lexicon::SchemaError::into_report)
                });
                match slot {
                    Ok(validator) => Ok(validator),
                    Err(report) => Err(report),
                }
            }
        }
    }
}

/// The one thing the engine asks of a single-document adapter.
///
/// `OdcsValidator`, `OdpsValidator` and `LexiconValidator` have the same shape
/// and no common trait — each implements [`Validator`] over its *own*
/// `Document` type, which is the right design for them and the wrong one for a
/// caller that wants to hold any of them. This is that caller's view: name and
/// text in, report out.
trait TextValidator {
    /// Validate a document given by name and text.
    fn validate_text(&self, id: &str, text: &str) -> ConformanceReport;
}

impl TextValidator for OdcsValidator {
    fn validate_text(&self, id: &str, text: &str) -> ConformanceReport {
        self.validate(&conform_odcs::Document::new(id, text))
    }
}

impl TextValidator for OdpsValidator {
    fn validate_text(&self, id: &str, text: &str) -> ConformanceReport {
        self.validate(&conform_odps::Document::new(id, text))
    }
}

impl TextValidator for LexiconValidator {
    fn validate_text(&self, id: &str, text: &str) -> ConformanceReport {
        self.validate(&conform_lexicon::Document::new(id, text))
    }
}

/// The catalogue, filtered by `--spec`, with every entry's bytes re-hashed.
///
/// Re-hashing every entry on every run — including entries no document in this
/// run is checked against — is deliberate. It costs a few kilobytes of SHA-256
/// and it is what lets the spec pane show the pin and the drift status of any
/// specification the moment it is selected, which is the "never more than two
/// keystrokes away" requirement in plan §4.2.
fn summarise(registry: &Registry, only: Option<&str>) -> Vec<SpecSummary> {
    registry
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| only.is_none_or(|id| entry.id == id))
        .map(|(index, entry)| SpecSummary::new(entry, registry.verify_entry(index, entry)))
        .collect()
}

/// The identifiers a registry does hold, for the help on an unknown `--spec`.
fn known_ids(registry: &Registry) -> String {
    let ids: Vec<&str> = registry
        .entries()
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    format!("this registry holds: {}", ids.join(", "))
}

/// Find `specs.toml`.
///
/// Explicitly, if `--registry` said where. Otherwise by walking up from the
/// current directory, the way every other repository-rooted tool finds its
/// configuration — so `conform` works from a subdirectory, which is where
/// people actually run things.
fn locate_registry(explicit: Option<&Path>) -> Result<PathBuf, Box<Diagnostic>> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    let start = env::current_dir().map_err(|error| {
        Box::new(Diagnostic::error(
            codes::REGISTRY_NOT_FOUND,
            Location::document("specs.toml"),
            format!("cannot read the current directory: {error}"),
        ))
    })?;

    for directory in start.ancestors() {
        let candidate = directory.join("specs.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(Box::new(
        Diagnostic::error(
            codes::REGISTRY_NOT_FOUND,
            Location::document("specs.toml"),
            format!(
                "no `specs.toml` in {} or any directory above it",
                start.display()
            ),
        )
        .with_help(
            "run from inside a repository that has one, or pass `--registry <path>` — without \
             a registry nothing can say where a vendored specification came from",
        ),
    ))
}

/// A run that could not happen, carrying the one diagnostic explaining it.
fn unusable(request: &Request, document: &str, diagnostic: Box<Diagnostic>) -> Run {
    Run {
        command: request.command,
        policy: request.policy,
        registry_path: document.to_owned(),
        specs: Vec::new(),
        documents: vec![DocumentOutcome::new(document, None, single(*diagnostic))],
        unusable: true,
    }
}

/// A report holding one diagnostic.
fn single(diagnostic: Diagnostic) -> ConformanceReport {
    let mut report = ConformanceReport::new();
    report.push(diagnostic);
    report
}

/// Every specification in the registry, by identifier, with the summary this
/// run computed for it. Used by the TUI's spec pane.
#[must_use]
pub fn by_id(run: &Run) -> BTreeMap<&str, &SpecSummary> {
    run.specs
        .iter()
        .map(|spec| (spec.id.as_str(), spec))
        .collect()
}
