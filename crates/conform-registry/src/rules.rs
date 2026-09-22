//! What a registry entry must record, and what happens when it does not.

use conform_core::{ConformanceReport, Diagnostic, Severity, SpecRef, Validator};

use crate::codes;
use crate::entry::{SpecEntry, is_moving_ref};
use crate::registry::{Registry, is_repo_relative, spec_ref};

impl Registry {
    /// Check the registry against its own rules, without touching any artefact.
    #[must_use]
    pub fn validate(&self) -> ConformanceReport {
        let mut report = ConformanceReport::new();

        for (index, entry) in self.entries().iter().enumerate() {
            self.validate_entry(index, entry, &mut report);
        }

        self.check_duplicate_ids(&mut report);
        report
    }

    fn validate_entry(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        for (field, value) in [("id", &entry.id), ("name", &entry.name)] {
            if value.trim().is_empty() {
                report.push(self.finding(
                    Severity::Error,
                    codes::BLANK_FIELD,
                    index,
                    field,
                    format!("`{field}` is blank, and it is one of the fields always knowable"),
                ));
            }
        }

        if entry.versions.is_empty() {
            report.push(self.finding(
                Severity::Error,
                codes::NO_VERSIONS,
                index,
                "pin",
                format!("`{}` has no pinned versions", entry.id),
            ));
        }

        for (vi, version) in entry.versions.iter().enumerate() {
            self.validate_version(index, vi, entry, version, report);
        }

        self.check_identity_gaps(index, entry, report);
        self.check_urls(index, entry, report);
        self.check_duplicate_versions(index, entry, report);
    }

    fn validate_version(
        &self,
        entry_index: usize,
        version_index: usize,
        entry: &SpecEntry,
        version: &crate::entry::PinnedVersion,
        report: &mut ConformanceReport,
    ) {
        for (field, value) in [
            ("vendored_path", &version.vendored_path),
            ("sha256", &version.sha256),
            ("fetched_at", &version.fetched_at),
        ] {
            if value.trim().is_empty() {
                report.push(self.version_finding(
                    Severity::Error,
                    codes::BLANK_FIELD,
                    entry_index,
                    version_index,
                    field,
                    format!(
                        "`{field}` is blank in `{}`{}",
                        entry.id,
                        version
                            .version
                            .as_deref()
                            .map_or(String::new(), |v| format!("@{v}"))
                    ),
                ));
            }
        }

        self.check_pin(entry_index, version_index, entry, version, report);
        self.check_custody(entry_index, version_index, entry, version, report);
        self.check_poll_url(entry_index, version_index, version, report);
    }

    fn check_pin(
        &self,
        entry_index: usize,
        version_index: usize,
        entry: &SpecEntry,
        version: &crate::entry::PinnedVersion,
        report: &mut ConformanceReport,
    ) {
        match version.pinned_ref.as_deref().map(str::trim) {
            Some(pin) if is_moving_ref(pin) => {
                report.push(
                    self.version_finding(
                        Severity::Error,
                        codes::MOVING_REF,
                        entry_index,
                        version_index,
                        "pinned_ref",
                        format!(
                            "`pinned_ref = \"{pin}\"` in `{}`{} names whatever upstream holds \
                             today, not one revision",
                            entry.id,
                            version
                                .version
                                .as_deref()
                                .map_or(String::new(), |v| format!("@{v}"))
                        ),
                    )
                    .with_help(
                        "record the release tag, version identifier or commit SHA the bytes were \
                         actually taken from",
                    ),
                );
            }
            Some("") => {
                report.push(self.version_finding(
                    Severity::Warning,
                    codes::UNPINNED,
                    entry_index,
                    version_index,
                    "pinned_ref",
                    "`pinned_ref` is blank: no upstream revision is recorded for these bytes",
                ));
            }
            Some(_) => {}
            None => {
                report.push(
                    self.version_finding(
                        Severity::Warning,
                        codes::UNPINNED,
                        entry_index,
                        version_index,
                        "pinned_ref",
                        "no `pinned_ref`: nothing records which upstream revision these bytes \
                         were taken from",
                    )
                    .with_help("record a tag, version identifier or commit SHA once one is known"),
                );
            }
        }
    }

    fn check_identity_gaps(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        let gaps = entry.provenance_gaps();

        for gap in &gaps {
            report.push(self.finding(
                Severity::Warning,
                codes::PROVENANCE_GAP,
                index,
                gap,
                format!("`{gap}` is not recorded for `{}`", entry.id),
            ));
        }

        // An unexplained gap at the entry level requires notes at the entry
        // level OR at at least one version level. Check both.
        if !gaps.is_empty() {
            let has_some_notes = entry.notes.as_deref().is_some_and(|n| !n.trim().is_empty())
                || entry
                    .versions
                    .iter()
                    .any(|v| v.notes.as_deref().is_some_and(|n| !n.trim().is_empty()));

            if !has_some_notes {
                report.push(
                    self.finding(
                        Severity::Error,
                        codes::UNEXPLAINED_GAP,
                        index,
                        "notes",
                        format!(
                            "`{}` leaves {} unrecorded and carries no `notes` saying why",
                            entry.id,
                            gaps.join(", ")
                        ),
                    )
                    .with_help(
                        "write what was looked at and what it did not say; a recorded unknown is \
                         usable evidence, an invented plausible value is not",
                    ),
                );
            }
        }
    }

    fn check_custody(
        &self,
        entry_index: usize,
        version_index: usize,
        _entry: &SpecEntry,
        version: &crate::entry::PinnedVersion,
        report: &mut ConformanceReport,
    ) {
        if !version.vendored_path.trim().is_empty() && !is_repo_relative(&version.vendored_path) {
            report.push(
                self.version_finding(
                    Severity::Error,
                    codes::PATH_NOT_REPO_RELATIVE,
                    entry_index,
                    version_index,
                    "vendored_path",
                    format!(
                        "`vendored_path = \"{}\"` is absolute or climbs out of the registry's \
                         directory",
                        version.vendored_path
                    ),
                )
                .with_help("vendor the artefact into this repository and record the path to it"),
            );
        }

        let sha = version.sha256.trim();
        if !sha.is_empty() && !is_sha256_hex(sha) {
            report.push(self.version_finding(
                Severity::Error,
                codes::MALFORMED_SHA256,
                entry_index,
                version_index,
                "sha256",
                format!("`sha256 = \"{sha}\"` is not 64 lower-case hex digits"),
            ));
        }

        let fetched = version.fetched_at.trim();
        if !fetched.is_empty() && !is_iso_date(fetched) {
            report.push(self.version_finding(
                Severity::Error,
                codes::MALFORMED_DATE,
                entry_index,
                version_index,
                "fetched_at",
                format!("`fetched_at = \"{fetched}\"` is not an ISO-8601 `YYYY-MM-DD` date"),
            ));
        }
    }

    fn check_urls(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        let fields = [
            ("homepage", entry.homepage.as_deref()),
            ("repository", entry.repository.as_deref()),
        ];

        for (field, value) in fields {
            let Some(url) = value.map(str::trim).filter(|url| !url.is_empty()) else {
                continue;
            };
            if !url.starts_with("https://") {
                report.push(self.finding(
                    Severity::Warning,
                    codes::INSECURE_URL,
                    index,
                    field,
                    format!("`{field} = \"{url}\"` is not an `https://` URL"),
                ));
            }
        }
    }

    fn check_poll_url(
        &self,
        entry_index: usize,
        version_index: usize,
        version: &crate::entry::PinnedVersion,
        report: &mut ConformanceReport,
    ) {
        if let Some(poll) = &version.poll {
            let url = poll.endpoint.trim();
            if !url.is_empty() && !url.starts_with("https://") {
                report.push(self.version_finding(
                    Severity::Warning,
                    codes::INSECURE_URL,
                    entry_index,
                    version_index,
                    "poll.endpoint",
                    format!("`poll.endpoint = \"{url}\"` is not an `https://` URL"),
                ));
            }
        }
    }

    fn check_duplicate_ids(&self, report: &mut ConformanceReport) {
        let entries = self.entries();
        for (index, entry) in entries.iter().enumerate() {
            let first = entries
                .iter()
                .position(|other| other.id == entry.id)
                .unwrap_or(index);
            if first != index {
                report.push(self.finding(
                    Severity::Error,
                    codes::DUPLICATE_ID,
                    index,
                    "id",
                    format!(
                        "`id = \"{}\"` is already used by the entry at /spec/{first}",
                        entry.id
                    ),
                ));
            }
        }
    }

    fn check_duplicate_versions(
        &self,
        entry_index: usize,
        entry: &SpecEntry,
        report: &mut ConformanceReport,
    ) {
        for (vi, version) in entry.versions.iter().enumerate() {
            let Some(ver_str) = version.version.as_deref() else {
                continue;
            };
            let first = entry
                .versions
                .iter()
                .position(|other| other.version.as_deref() == Some(ver_str))
                .unwrap_or(vi);
            if first != vi {
                report.push(self.version_finding(
                    Severity::Error,
                    codes::DUPLICATE_VERSION,
                    entry_index,
                    vi,
                    "version",
                    format!(
                        "`version = \"{ver_str}\"` is already used by pin/{first} in `{}`",
                        entry.id
                    ),
                ));
            }
        }
    }

    fn finding(
        &self,
        severity: Severity,
        code: &str,
        index: usize,
        field: &str,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic::new(
            severity,
            code,
            self.entry_location(index, field),
            message.into(),
        )
        .with_spec_ref(spec_ref())
    }

    fn version_finding(
        &self,
        severity: Severity,
        code: &str,
        entry_index: usize,
        version_index: usize,
        field: &str,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic::new(
            severity,
            code,
            self.version_location(entry_index, version_index, field),
            message.into(),
        )
        .with_spec_ref(spec_ref())
    }
}

/// The registry's own rules, as a [`Validator`] like any other in this family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProvenanceRules;

impl Validator for ProvenanceRules {
    type Document = Registry;

    fn spec(&self) -> SpecRef {
        spec_ref()
    }

    fn validate(&self, registry: &Registry) -> ConformanceReport {
        registry.validate()
    }
}

fn is_sha256_hex(candidate: &str) -> bool {
    candidate.len() == 64
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_iso_date(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, byte)| match i {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}
