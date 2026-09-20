//! Accumulating diagnostics while remembering which document is being read.

use std::path::Path;

use conform_core::{ConformanceReport, Diagnostic, DiagnosticCode, Location, Severity};
use okf_core::Concept;

use crate::spec_ref;

/// Accumulates diagnostics, remembering which concept is being examined so
/// each rule can raise one without restating the path and id.
///
/// The [`Location`] it builds carries **both** of the things the report this
/// crate replaces kept side by side:
///
/// - [`Location::document`] is the file, bundle-relative. Relative on purpose:
///   an absolute path leaks the directory the run happened in, and two runs of
///   the same bundle from different checkouts would then not compare equal.
/// - [`Location::pointer`] is the concept id, when the finding is about a
///   concept rather than about an index, a log, or a file that did not parse.
///
/// The id is derivable from the path — it is the path without `.md` — so
/// carrying both is mild redundancy. It is carried anyway because a consumer
/// of the old report keyed on the concept id, and making that consumer
/// re-derive an identifier the producer already had is how a migration loses
/// something nobody notices until later.
///
/// `Location::line` stays `None` throughout, matching the implementation this
/// replaces: the rules that know a line number (`OKFL03`, `OKFL04`, `OKFL08`)
/// state it in their message, exactly as they did before. Lifting those into
/// the location is an improvement, and deliberately not made here — this is a
/// migration, and a migration that also improves the output cannot be checked
/// against the thing it migrated.
pub(crate) struct Cx<'a> {
    diagnostics: Vec<Diagnostic>,
    concept: Option<String>,
    path: Option<String>,
    root: &'a Path,
}

impl<'a> Cx<'a> {
    pub(crate) fn new(root: &'a Path) -> Self {
        Self {
            diagnostics: Vec::new(),
            concept: None,
            path: None,
            root,
        }
    }

    /// Point subsequent diagnostics at `concept`.
    pub(crate) fn at(&mut self, concept: &Concept) {
        self.concept = Some(concept.id.to_string());
        self.path = Some(self.relative(&concept.path));
    }

    /// Point subsequent diagnostics at a file that is not a concept — an
    /// index, a log, or a document that failed to parse.
    pub(crate) fn at_file(&mut self, path: &Path) {
        self.concept = None;
        self.path = Some(self.relative(path));
    }

    /// Stop pointing at anything in particular.
    pub(crate) fn at_bundle(&mut self) {
        self.concept = None;
        self.path = None;
    }

    /// Bundle-relative, so a report does not leak the absolute path it was run
    /// from and two runs of the same bundle compare equal.
    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// Where a diagnostic raised right now applies.
    ///
    /// With no file in hand the bundle root is named. A `Location` has to name
    /// *some* document, and a finding about the bundle as a whole belongs to
    /// the bundle rather than to whichever file the walk last touched.
    fn location(&self) -> Location {
        let document = self
            .path
            .clone()
            .unwrap_or_else(|| self.root.display().to_string());
        let location = Location::document(document);
        match &self.concept {
            Some(concept) => location.with_pointer(concept.clone()),
            None => location,
        }
    }

    fn push(
        &mut self,
        severity: Severity,
        code: impl Into<DiagnosticCode>,
        section: Option<&str>,
        message: String,
    ) {
        let spec = match section {
            Some(section) => spec_ref().with_section(section),
            None => spec_ref(),
        };
        self.diagnostics
            .push(Diagnostic::new(severity, code, self.location(), message).with_spec_ref(spec));
    }

    pub(crate) fn err(
        &mut self,
        code: &'static str,
        section: Option<&str>,
        message: impl Into<String>,
    ) {
        self.push(Severity::Error, code, section, message.into());
    }

    pub(crate) fn warn(
        &mut self,
        code: &'static str,
        section: Option<&str>,
        message: impl Into<String>,
    ) {
        self.push(Severity::Warning, code, section, message.into());
    }

    pub(crate) fn info(
        &mut self,
        code: &'static str,
        section: Option<&str>,
        message: impl Into<String>,
    ) {
        self.push(Severity::Info, code, section, message.into());
    }

    /// Everything collected, as a report in reading order: errors first, then
    /// warnings, then info.
    ///
    /// Sorted rather than relied upon — the traversal order above is an
    /// implementation detail, while this ordering is what a reader sees first
    /// and what a CI log diff compares — but the ordering itself is
    /// `conform-core`'s
    /// [`sort_by_severity`](ConformanceReport::sort_by_severity), not a second
    /// copy of the rule kept here. It is format-agnostic, so a copy in this
    /// crate would only be one more thing to keep in step. Its sort is stable,
    /// so within one severity the bundle's own order still survives.
    pub(crate) fn finish(self) -> ConformanceReport {
        let mut report: ConformanceReport = self.diagnostics.into_iter().collect();
        report.sort_by_severity();
        report
    }
}
