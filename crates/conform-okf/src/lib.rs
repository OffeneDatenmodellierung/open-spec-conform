//! Conformance and hygiene checking for an **Open Knowledge Format** v0.2
//! bundle, reported as [`conform_core`] diagnostics.
//!
//! Two checks by default and a third on request, deliberately separate types,
//! because they answer different questions:
//!
//! | Type | Asks | Gates on | Needs |
//! |---|---|---|---|
//! | [`OkfConformance`] | is this bundle *OKF*? | errors | — |
//! | [`OkfHygiene`] | is this bundle *good* OKF? | nothing | — |
//! | `OkfSyntax` | does the code in it *parse*? | nothing | feature `syntax` |
//!
//! That split is not a convention this crate maintains by hand. It is
//! [`conform_core`]'s report-versus-gate separation: both types implement
//! [`Validator`], so both *report* through
//! [`validate`](Validator::validate) — which cannot fail — and both *gate*
//! through [`check`](Validator::check), against a
//! [`GatePolicy`](conform_core::GatePolicy) the caller names. Each type states
//! the policy it is meant to be read under in its own `gate_policy`, and the
//! two differ because the questions do.
//!
//! # Why the rules are here rather than depended on
//!
//! Upstream's `okf-validator` does this job and was adopted for a while
//! elsewhere in this estate. It could not be kept: none of its dependencies is
//! optional and it syntax-checks fenced code blocks in several languages, so
//! taking it means taking `rustpython-parser` — 61 crates, `LGPL-3.0-only`
//! through the `malachite` tree, and six unmaintained advisories whose own
//! text says no safe upgrade exists. `cargo deny` refuses that on both counts,
//! and admitting a licence merely to turn CI green is not a thing this
//! repository does.
//!
//! What is rebuilt here is the **structural** half, which is the half that is
//! about OKF. It costs no dependency: every rule is expressed over
//! [`okf_core`]'s own model, which is one crate with zero transitive
//! dependencies.
//!
//! # This is not a re-derivation of the specification
//!
//! Nothing here parses OKF. Frontmatter, trust tiers, actor classes, links,
//! footnotes, headings, computations and concept ids all come from
//! [`okf_core`] — an independent reading of the same specification by an
//! author who is not us — and the functions here only ask questions *about*
//! the model it returns. When the specification changes, the parsing follows
//! upstream and only the questions are ours.
//!
//! Re-deriving the format is how two readers of one specification end up
//! disagreeing, and an independent implementation is worth more than a
//! faithful one: a reader of our own construction, run over our own output,
//! can only catch a mistake we did not make twice.
//!
//! # Code syntax is not checked by default
//!
//! Whether a fenced `sql` block is valid SQL says nothing about whether a
//! bundle is valid OKF — a document full of pseudocode is perfectly
//! conformant. Folding the two together is what made upstream's validator
//! expensive, and it is also what makes it noisy. The two code-parsing checks
//! are therefore **absent by default**: with no feature enabled, this crate's
//! dependency tree is [`conform_core`] and [`okf_core`] and stops, and their
//! absence is the only intended behavioural difference from upstream over the
//! published corpus.
//!
//! They are no longer *unavailable*, which is what this paragraph used to say.
//! Enabling the `syntax` feature adds [`OkfSyntax`], a third [`Validator`] over
//! the same [`Bundle`], backed by [`conform_okf_syntax`] — a crate written for
//! exactly this job, whose every code parser is itself optional so that a
//! consumer chooses what it is willing to compile:
//!
//! | Feature | Adds | Costs |
//! |---|---|---|
//! | `syntax` | `OkfSyntax`; JSON, YAML and shell quoting | one pure-Rust crate, no parser |
//! | `syntax-sql` | SQL, via `sqlparser` | ~17 crates, two compiling assembly |
//! | `syntax-grammars` | Python, JavaScript, TypeScript, Rust, Bash | tree-sitter grammars, which compile C |
//!
//! `syntax` changes what is *reported* and deliberately not what either of the
//! other two checks reports: [`OkfConformance`] and [`OkfHygiene`] are
//! byte-identical in every feature configuration, because the rules are added
//! as a separate validator rather than folded into an existing one. See
//! [`OkfSyntax`] for why that distinction is the one that matters.
//!
//! # Determinism
//!
//! `stale_after` is checked for **syntax** and never against the clock, so a
//! bundle that validates today validates tomorrow. A check whose result
//! depends on when it ran cannot be a gate, and this one is used as one.
//!
//! Nothing here performs network access either. A `resource:` naming
//! `https://…` is reported as [`Resolution::NotInspected`], never as absent:
//! see [`resolve_resource`].
//!
//! # Messages quote the bundle, and this crate does not escape them
//!
//! A diagnostic's `message` interpolates values a *peer* wrote — a title, an
//! actor id, a path, a frontmatter scalar — because a finding that does not
//! quote what it found is not actionable. Those values are not sanitised here.
//! A bundle can therefore carry a bidirectional override or a zero-width run
//! into a message, and a renderer that writes it straight to a terminal will
//! show a line the bundle rewrote.
//!
//! That is the renderer's boundary, not this crate's, and the split is
//! deliberate: escaping is not idempotent — a `\` doubles on every pass — so
//! it has to happen exactly once, at the point of display, and a library that
//! escaped eagerly would make that impossible to guarantee. The fields stay
//! raw so a caller that wants the path in order to *open* it still gets the
//! path.
//!
//! # Example
//!
//! ```
//! use conform_core::{GatePolicy, Severity, Validator};
//! use conform_okf::{Bundle, OkfConformance, OkfHygiene};
//!
//! let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/okf-upstream/acme_retail");
//! let bundle = Bundle::load(root)?;
//!
//! let conformance = OkfConformance.validate(&bundle);
//! let hygiene = OkfHygiene.validate(&bundle);
//!
//! // Hygiene has opinions about a bundle the specification is content with.
//! assert!(hygiene.len() > conformance.len());
//!
//! // And it cannot fail anybody's build, because that is not its job.
//! assert_eq!(hygiene.count(Severity::Error), 0);
//! assert!(!hygiene.should_gate(OkfHygiene.gate_policy()));
//! # Ok::<(), okf_core::BundleError>(())
//! ```

pub mod codes;
mod conformance;
mod context;
mod hygiene;
mod index;
mod resolve;
#[cfg(feature = "syntax")]
mod syntax;

use std::fmt;
use std::path::Path;

use conform_core::{
    ConformanceReport, Diagnostic, GatePolicy, GateVerdict, Location, SpecRef, Validator,
};

pub use conformance::OkfConformance;
pub use hygiene::OkfHygiene;
pub use resolve::{bundle_relative, resolve_resource};
#[cfg(feature = "syntax")]
pub use syntax::OkfSyntax;

/// `conform-okf-syntax`, re-exported, when the `syntax` feature is on.
///
/// A **public dependency** for the same reason [`okf_core`] is: a caller that
/// summarises an [`OkfSyntax`] report has to be able to say which languages
/// this build could actually read, and
/// [`checkable_languages`](conform_okf_syntax::checkable_languages) is the
/// honest answer. Making them find a matching version of a second crate to ask
/// a question about this one's report would be a poor trade.
#[cfg(feature = "syntax")]
pub use conform_okf_syntax;

/// `okf-core`, re-exported. It is a **public dependency**: [`Bundle`] is the
/// document type both validators here take, so a consumer cannot use this
/// crate without it and should not have to name a second version of it.
pub use okf_core;
pub use okf_core::Bundle;

/// The version of the specification this crate implements.
///
/// A bundle declaring a different one is read best-effort and noted
/// ([`codes::UNIMPLEMENTED_VERSION`]) rather than refused, which is what §12
/// asks of a consumer.
pub const OKF_VERSION: &str = "0.2";

/// The identifier this crate's diagnostics are raised under.
///
/// It is the `id` of the `okf` entry in the repository's `specs.toml`, so a
/// [`Diagnostic::spec_ref`] leads to the provenance record for the bytes the
/// rule was written against: upstream repository, pinned commit, licence.
/// That link is the point of carrying a `SpecRef` at all.
pub const SPEC_ID: &str = "okf";

/// The reference this crate's diagnostics are raised under, with no clause
/// attached.
///
/// ```
/// assert_eq!(conform_okf::spec_ref().to_string(), "okf@0.2");
/// ```
#[must_use]
pub fn spec_ref() -> SpecRef {
    SpecRef::new(SPEC_ID).with_version(OKF_VERSION)
}

/// Which of the two checks produced a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Check {
    /// [`OkfConformance`] — is this bundle OKF?
    Validate,
    /// [`OkfHygiene`] — is this bundle good OKF?
    Lint,
}

impl Check {
    /// The stable, machine-readable name of this check.
    ///
    /// `validate` and `lint`, the names the report this crate replaces wrote
    /// into its `check` field and the names the commands built on it use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::Lint => "lint",
        }
    }

    /// The policy this check is meant to be gated under.
    #[must_use]
    pub const fn gate_policy(self) -> GatePolicy {
        match self {
            Self::Validate => OkfConformance.gate_policy(),
            Self::Lint => OkfHygiene.gate_policy(),
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One check over one bundle: the diagnostics, and the three facts about the
/// run that are not diagnostics.
///
/// [`ConformanceReport`] holds findings and nothing else, which is correct for
/// a type every adapter shares. The three fields here are what a *bundle*
/// check additionally knows, and the one that earns its place is
/// [`concepts`](Self::concepts): a report with no findings over a bundle with
/// no concepts is not a clean bill of health, and without the count those two
/// outcomes are the same empty list.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BundleReport {
    root: String,
    check: Check,
    concepts: usize,
    report: ConformanceReport,
}

impl BundleReport {
    /// The bundle root, as the caller named it.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Which check this was.
    #[must_use]
    pub const fn check(&self) -> Check {
        self.check
    }

    /// How many concepts were examined.
    #[must_use]
    pub const fn concepts(&self) -> usize {
        self.concepts
    }

    /// The diagnostics: errors first, then warnings, then info. Within one
    /// severity, bundle order is preserved.
    #[must_use]
    pub const fn report(&self) -> &ConformanceReport {
        &self.report
    }

    /// Take the diagnostics, to merge into a larger report.
    #[must_use]
    pub fn into_report(self) -> ConformanceReport {
        self.report
    }

    /// Apply a gating policy.
    #[must_use]
    pub fn gate(&self, policy: GatePolicy) -> GateVerdict {
        self.report.gate(policy)
    }

    /// `true` when nothing rose to `error`.
    ///
    /// Exactly the question the report this crate replaces answered with
    /// `errors == 0`, and it is kept because callers ask it. Warnings
    /// deliberately do not fail: §11 tells a consumer not to reject a document
    /// over a soft-guidance deviation, and a check that failed on one would be
    /// unusable against real third-party bundles.
    #[must_use]
    pub fn passed(&self) -> bool {
        !self.report.should_gate(GatePolicy::default())
    }
}

/// Check a bundle for conformance with the OKF v0.2 specification.
///
/// Deterministic — see the crate documentation.
///
/// # Errors
///
/// [`LoadError`] if the path is not a loadable OKF bundle.
pub fn validate_report(root: impl AsRef<Path>) -> Result<BundleReport, LoadError> {
    report_for(Check::Validate, root.as_ref())
}

/// Check a bundle against the hygiene rules.
///
/// # Errors
///
/// [`LoadError`] if the path is not a loadable OKF bundle.
pub fn lint_report(root: impl AsRef<Path>) -> Result<BundleReport, LoadError> {
    report_for(Check::Lint, root.as_ref())
}

/// The bundle-in-hand half of [`validate_report`], so a caller that has
/// already loaded a [`Bundle`] pays for the directory walk once.
#[must_use]
pub fn validate_bundle(bundle: &Bundle) -> BundleReport {
    BundleReport {
        root: bundle.root().display().to_string(),
        check: Check::Validate,
        concepts: bundle.concepts().len(),
        report: OkfConformance.validate(bundle),
    }
}

/// The bundle-in-hand half of [`lint_report`].
#[must_use]
pub fn lint_bundle(bundle: &Bundle) -> BundleReport {
    BundleReport {
        root: bundle.root().display().to_string(),
        check: Check::Lint,
        concepts: bundle.concepts().len(),
        report: OkfHygiene.validate(bundle),
    }
}

fn report_for(check: Check, root: &Path) -> Result<BundleReport, LoadError> {
    let bundle = load(root)?;
    Ok(match check {
        Check::Validate => validate_bundle(&bundle),
        Check::Lint => lint_bundle(&bundle),
    })
}

/// Load a bundle, reporting a failure the way every other validator in this
/// family reports one.
///
/// # Errors
///
/// [`LoadError`] if `root` is not a directory, or the walk hits an I/O
/// failure. A *file* that fails to parse is not a load failure: it is recorded
/// on the bundle and raised as [`codes::UNREADABLE_DOCUMENT`] by
/// [`OkfConformance`], because a bundle with one broken document in it is
/// still a bundle and the other documents still have findings worth having.
pub fn load(root: impl AsRef<Path>) -> Result<Bundle, LoadError> {
    let root = root.as_ref();
    Bundle::load(root).map_err(|error| {
        LoadError::single(
            Diagnostic::error(
                codes::BUNDLE_UNREADABLE,
                Location::document(root.display().to_string()),
                format!("not a readable OKF bundle: {error}"),
            )
            .with_spec_ref(spec_ref()),
        )
    })
}

/// A bundle that could not be loaded at all.
///
/// Carries a [`ConformanceReport`] rather than a string, so a caller renders a
/// load failure through exactly the same path as a conformance failure — the
/// reason this crate depends on `conform-core` in the first place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    report: ConformanceReport,
}

impl LoadError {
    fn single(diagnostic: Diagnostic) -> Self {
        let mut report = ConformanceReport::new();
        report.push(diagnostic);
        Self { report }
    }

    /// The diagnostics explaining the failure.
    #[must_use]
    pub const fn report(&self) -> &ConformanceReport {
        &self.report
    }

    /// Take the diagnostics, to merge into a larger report.
    #[must_use]
    pub fn into_report(self) -> ConformanceReport {
        self.report
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.report.diagnostics() {
            [] => f.write_str("bundle could not be loaded"),
            [first, rest @ ..] => {
                write!(f, "{first}")?;
                if !rest.is_empty() {
                    write!(f, " (and {} more)", rest.len())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for LoadError {}
