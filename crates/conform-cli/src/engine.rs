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

use conform_core::{
    ConformanceReport, Diagnostic, GatePolicy, Location, Severity, SpecRef, Validator,
};
use conform_lexicon::LexiconValidator;
use conform_odcs::OdcsValidator;
use conform_odps::OdpsValidator;
use conform_registry::{PinnedVersion, Registry, SpecEntry};

use crate::codes;
use crate::corpus;
use crate::discover::{self, Target};
use crate::embedded;
use crate::model::{
    COMMAND_LINE, Command, DocumentOutcome, FileStandard, RegistryOrigin, Run, SpecSummary,
    Standard,
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
    let origin = locate_registry(request.registry.as_deref());
    let display_path = match origin.path() {
        Some(path) => path.display().to_string(),
        None => embedded::REGISTRY_NAME.to_owned(),
    };

    let loaded = match origin.path() {
        Some(path) => Registry::load_path(path),
        None => Registry::load_str(embedded::REGISTRY_TOML, embedded::REGISTRY_NAME),
    };
    let registry = match loaded {
        Ok(registry) => registry,
        Err(error) => {
            return Run {
                command: request.command,
                policy: request.policy,
                registry_path: display_path.clone(),
                registry_origin: origin,
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
        specs: summarise(&registry, &origin, request.spec.as_deref()),
        registry_origin: origin,
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

    let mut validators = Validators::new(registry, &run.registry_origin);
    let mut outcomes: Vec<DocumentOutcome> = discovery
        .targets
        .iter()
        .map(|target| check(target, &mut validators))
        .collect();

    // The corpus pass, after every document has been checked on its own. It
    // answers the two questions no adapter can — does an ODPS `contractId`
    // name a contract that is here, and do two contracts claim one `id` — and
    // it raises warnings and information only, so a document's verdict is
    // exactly what its adapter said it was.
    for (outcome, findings) in outcomes.iter_mut().zip(corpus::check(&discovery.targets)) {
        if findings.is_empty() {
            continue;
        }
        outcome.report.extend(findings);
        // `DocumentOutcome::new` ordered the report by severity and this has
        // just appended to it. Re-ordering keeps every renderer's "errors
        // first" promise true of the merged list rather than of the list as it
        // was a moment ago.
        outcome.report.sort_by_severity();
    }

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

/// An OKF bundle: conformance and hygiene, merged — and code syntax too, when
/// this binary was built with a `syntax` feature.
///
/// All of them, because they are all about this bundle and a reader wants one
/// list. They stay distinguishable by severity, which is the split that
/// matters: nothing hygiene or syntax raises is ever an error, so merging them
/// cannot change whether the bundle gates.
///
/// The third one is the only place in this crate that a feature changes what a
/// run reports, and it is the honest shape of that change: `conform-okf`
/// decides whether `OkfSyntax` exists at all, this function decides nothing,
/// and the feature here is a forwarding address with no rules behind it. A
/// build without it is not a build that checked the code and found it clean —
/// `conform_okf::conform_okf_syntax::checkable_languages` is what a summary
/// should say instead.
fn bundle_report(path: &Path) -> ConformanceReport {
    match conform_okf::load(path) {
        Ok(bundle) => {
            let mut report = conform_okf::validate_bundle(&bundle).into_report();
            report.merge(conform_okf::lint_bundle(&bundle).into_report());
            #[cfg(feature = "syntax")]
            report.merge(conform_okf::OkfSyntax.validate(&bundle));
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
    origin: &'r RegistryOrigin,
    odcs: Option<Result<OdcsValidator, ConformanceReport>>,
    odps: Option<Result<OdpsValidator, ConformanceReport>>,
    odcl: Option<Result<LexiconValidator, ConformanceReport>>,
}

impl<'r> Validators<'r> {
    const fn new(registry: &'r Registry, origin: &'r RegistryOrigin) -> Self {
        Self {
            registry,
            origin,
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
                    if self.origin.is_embedded() {
                        embedded_schema(
                            self.registry,
                            "odcs",
                            conform_odcs::codes::SCHEMA_PROVENANCE_FAILED,
                        )
                        .and_then(|s| {
                            OdcsValidator::from_schema_str(s.text, s.spec, s.provenance, &s.name)
                                .map_err(conform_odcs::SchemaError::into_report)
                        })
                    } else {
                        OdcsValidator::from_registry(self.registry)
                            .map_err(conform_odcs::SchemaError::into_report)
                    }
                });
                match slot {
                    Ok(validator) => Ok(validator),
                    Err(report) => Err(report),
                }
            }
            FileStandard::Odps => {
                let slot = self.odps.get_or_insert_with(|| {
                    if self.origin.is_embedded() {
                        embedded_schema(
                            self.registry,
                            "odps",
                            conform_odps::codes::SCHEMA_PROVENANCE_FAILED,
                        )
                        .and_then(|s| {
                            OdpsValidator::from_schema_str(s.text, s.spec, s.provenance, &s.name)
                                .map_err(conform_odps::SchemaError::into_report)
                        })
                    } else {
                        OdpsValidator::from_registry(self.registry)
                            .map_err(conform_odps::SchemaError::into_report)
                    }
                });
                match slot {
                    Ok(validator) => Ok(validator),
                    Err(report) => Err(report),
                }
            }
            FileStandard::Odcl => {
                let slot = self.odcl.get_or_insert_with(|| {
                    if self.origin.is_embedded() {
                        embedded_schema(
                            self.registry,
                            "odcl",
                            conform_lexicon::codes::SCHEMA_PROVENANCE_FAILED,
                        )
                        .and_then(|s| {
                            LexiconValidator::from_schema_str(s.text, s.spec, s.provenance, &s.name)
                                .map_err(conform_lexicon::SchemaError::into_report)
                        })
                    } else {
                        LexiconValidator::from_registry(self.registry)
                            .map_err(conform_lexicon::SchemaError::into_report)
                    }
                });
                match slot {
                    Ok(validator) => Ok(validator),
                    Err(report) => Err(report),
                }
            }
        }
    }
}

/// What an adapter needs to be built from bytes, once it has earned them.
struct EmbeddedSchema {
    /// The schema text, compiled into this binary.
    text: &'static str,
    /// Which specification, at which version, the report will cite.
    spec: SpecRef,
    /// The sentence every report carries under the adapter's `…904` code.
    provenance: String,
    /// What a finding names as the schema it was checked against.
    name: String,
}

/// Build the inputs for an embedded validator, refusing unless the bytes pass
/// the digest gate.
///
/// This is `OdcsValidator::from_registry` and its two siblings, step for step,
/// with the filesystem read replaced by a lookup in [`embedded::ARTEFACTS`]:
/// find the entry, **re-hash the bytes against the digest the registry
/// records**, refuse on failure under the adapter's own
/// `SCHEMA_PROVENANCE_FAILED` code with the registry's diagnostic kept
/// underneath, and only then hand the text over.
///
/// The order is the point. A validator built before the gate is a validator
/// that issues verdicts against bytes nobody checked, and an embedded copy
/// that skipped the check would be a *worse* false green than the on-disk one
/// it replaced — the reader cannot go and look at the file to see for
/// themselves, because there is no file.
fn embedded_schema(
    registry: &Registry,
    spec_id: &str,
    provenance_failed: &'static str,
) -> Result<EmbeddedSchema, ConformanceReport> {
    let source = Location::document(registry.source().clone());

    let Some(entry_index) = registry.entries().iter().position(|e| e.id == spec_id) else {
        return Err(single(
            Diagnostic::error(
                provenance_failed,
                source,
                format!(
                    "the embedded registry holds no `{spec_id}` entry, so there is no schema to \
                     validate against"
                ),
            )
            .with_help(
                "pass `--registry <path>` to check against a repository whose catalogue has the \
                 entry",
            ),
        ));
    };
    let entry = &registry.entries()[entry_index];
    let version_index = entry.default_version_index();
    let version = entry.default_version();

    let Some(text) = embedded::artefact(&version.vendored_path) else {
        return Err(single(
            Diagnostic::error(
                provenance_failed,
                source,
                format!(
                    "the embedded registry records `{}` for `{spec_id}`, and this binary carries \
                     no copy of those bytes",
                    version.vendored_path
                ),
            )
            .with_help(
                "pass `--registry <path>` — an embedded catalogue can only speak for the bytes \
                 compiled into it",
            ),
        ));
    };

    let label = version
        .version
        .as_deref()
        .map_or_else(|| entry.id.clone(), |v| format!("{}@{v}", entry.id));

    // The gate. Nothing below this line runs against bytes that did not hash to
    // what the registry records for them.
    let integrity = conform_registry::verify_bytes(
        entry_index,
        version_index,
        &label,
        version,
        text.as_bytes(),
        embedded::artefact_name(&version.vendored_path),
    );
    if integrity.severity >= Severity::Error {
        let mut report = single(
            Diagnostic::error(
                provenance_failed,
                integrity.location.clone(),
                format!(
                    "refusing to validate against the embedded `{spec_id}` schema: its \
                     provenance check failed"
                ),
            )
            .with_help(
                "a verdict issued against a schema nobody can trace to a published standard is \
                 not a conformance verdict — this build's bytes and its catalogue disagree",
            ),
        );
        report.push(integrity);
        return Err(report);
    }

    let mut spec = SpecRef::new(entry.id.clone());
    if let Some(v) = &version.version {
        spec = spec.with_version(v.clone());
    }

    // Says *embedded*, and says what that does not cover. A sentence reading
    // the same as the on-disk one would be claiming a check this build cannot
    // perform: both sides of the comparison were frozen into the executable at
    // the same moment, so it speaks for this binary and for no file on disk.
    let provenance = match &version.pinned_ref {
        Some(pinned) => format!(
            "bytes of `{}` embedded in {} {} at publish time and re-hashed here against the \
             digest its catalogue records for upstream pin `{pinned}`; the catalogue is embedded \
             alongside them, so this says nothing about any `specs.toml` on disk",
            version.vendored_path,
            crate::human::PACKAGE_NAME,
            crate::human::TOOL_VERSION,
        ),
        None => format!(
            "bytes of `{}` embedded in {} {} at publish time and re-hashed here against the \
             digest its catalogue records; the entry records no upstream pin, and the catalogue \
             is embedded alongside the bytes, so this says nothing about any `specs.toml` on disk",
            version.vendored_path,
            crate::human::PACKAGE_NAME,
            crate::human::TOOL_VERSION,
        ),
    };

    let name = embedded::artefact_name(&version.vendored_path);
    Ok(EmbeddedSchema {
        text,
        spec,
        provenance,
        name,
    })
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

/// Parse a `--spec` value into an id and an optional `@version`.
///
/// `"ossie"` → `("ossie", None)`.
/// `"ossie@0.2.0.dev0"` → `("ossie", Some("0.2.0.dev0"))`.
fn parse_spec(spec: &str) -> (&str, Option<&str>) {
    match spec.split_once('@') {
        Some((id, version)) => (id, Some(version)),
        None => (spec, None),
    }
}

/// The catalogue, filtered by `--spec`, with every version's bytes re-hashed.
///
/// Re-hashing every version on every run — including entries no document in this
/// run is checked against — is deliberate. It costs a few kilobytes of SHA-256
/// and it is what lets the spec pane show the pin and the drift status of any
/// specification the moment it is selected, which is the "never more than two
/// keystrokes away" requirement in plan §4.2.
fn summarise(registry: &Registry, origin: &RegistryOrigin, only: Option<&str>) -> Vec<SpecSummary> {
    let (filter_id, filter_version) = only.map(parse_spec).unzip();

    registry
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| filter_id.is_none_or(|id| entry.id == id))
        .flat_map(|(entry_index, entry)| {
            let default_vi = entry.default_version_index();
            entry
                .versions
                .iter()
                .enumerate()
                .filter(move |(_, version)| {
                    filter_version
                        .flatten()
                        .is_none_or(|fv| version.version.as_deref() == Some(fv))
                })
                .map(move |(version_index, version)| {
                    SpecSummary::new(
                        entry,
                        version,
                        version_index == default_vi,
                        verify_artefact(
                            registry,
                            origin,
                            entry_index,
                            version_index,
                            entry,
                            version,
                        ),
                    )
                })
        })
        .collect()
}

/// Re-hash one version's artefact, wherever this run's artefacts live.
///
/// On disk this is `conform-registry`'s own `verify_version`. Embedded, it is
/// that crate's `verify_bytes` — the *same* comparison, deliberately: the
/// digest gate is the reason a verdict from this tool means anything, and two
/// implementations of it would be one implementation and one place for it to
/// quietly become decoration.
///
/// An artefact the registry records and this build does not carry is reported
/// as [`ARTEFACT_MISSING`](conform_registry::codes::ARTEFACT_MISSING), which is
/// what it is. A pass would be a lie about bytes nobody has.
fn verify_artefact(
    registry: &Registry,
    origin: &RegistryOrigin,
    entry_index: usize,
    version_index: usize,
    entry: &SpecEntry,
    version: &PinnedVersion,
) -> Diagnostic {
    if !origin.is_embedded() {
        return registry.verify_version(entry_index, version_index, version);
    }
    let label = version
        .version
        .as_deref()
        .map_or_else(|| entry.id.clone(), |v| format!("{}@{v}", entry.id));
    match embedded::artefact(&version.vendored_path) {
        Some(text) => conform_registry::verify_bytes(
            entry_index,
            version_index,
            &label,
            version,
            text.as_bytes(),
            embedded::artefact_name(&version.vendored_path),
        ),
        None => Diagnostic::error(
            conform_registry::codes::ARTEFACT_MISSING,
            Location::document(embedded::artefact_name(&version.vendored_path))
                .with_pointer(format!("/spec/{entry_index}/pin/{version_index}/sha256")),
            format!(
                "`{label}` records `vendored_path = \"{}\"`, and this binary carries no embedded \
                 copy of it",
                version.vendored_path
            ),
        )
        .with_help(
            "pass `--registry <path>` to check against a repository that has the artefact — an \
             embedded catalogue can only speak for the bytes compiled into it",
        )
        .with_spec_ref(conform_registry::spec_ref()),
    }
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
fn locate_registry(explicit: Option<&Path>) -> RegistryOrigin {
    if let Some(path) = explicit {
        return RegistryOrigin::Explicit(path.to_path_buf());
    }

    // A working directory that cannot be read is not a reason to refuse: it
    // means the search cannot happen, and the search is only the second of
    // three answers. Falling through to the embedded catalogue is what the
    // precedence says to do when nothing on disk answers, and this is one of
    // the ways nothing on disk answers.
    if let Ok(start) = env::current_dir() {
        for directory in start.ancestors() {
            let candidate = directory.join("specs.toml");
            if candidate.is_file() {
                return RegistryOrigin::Discovered(candidate);
            }
        }
    }

    RegistryOrigin::Embedded
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
