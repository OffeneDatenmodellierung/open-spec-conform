//! What a registry entry must record, and what happens when it does not.
//!
//! Every rule here is about one question: could a reader, months from now and
//! with no access to whoever vendored the bytes, answer *where did this come
//! from and is it still what it claims to be?* A rule earns its place by
//! closing one way of leaving that question unanswerable.

use conform_core::{ConformanceReport, Diagnostic, Severity, SpecRef, Validator};

use crate::codes;
use crate::entry::{SpecEntry, is_moving_ref};
use crate::registry::{Registry, is_repo_relative, spec_ref};

impl Registry {
    /// Check the registry against its own rules, without touching any artefact.
    ///
    /// Pure: no filesystem, no network, no clock. Use
    /// [`verify`](Registry::verify) for the question of whether the bytes on
    /// disk still match.
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
        for (field, value) in [
            ("id", &entry.id),
            ("name", &entry.name),
            ("vendored_path", &entry.vendored_path),
            ("sha256", &entry.sha256),
            ("fetched_at", &entry.fetched_at),
        ] {
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

        self.check_pin(index, entry, report);
        self.check_gaps(index, entry, report);
        self.check_custody(index, entry, report);
        self.check_urls(index, entry, report);
    }

    /// The rule the crate is named for.
    fn check_pin(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        match entry.pinned_ref.as_deref().map(str::trim) {
            Some(pin) if is_moving_ref(pin) => {
                report.push(
                    self.finding(
                        Severity::Error,
                        codes::MOVING_REF,
                        index,
                        "pinned_ref",
                        format!(
                            "`pinned_ref = \"{pin}\"` names whatever upstream holds today, not one \
                             revision, so nothing can ever check this copy against it"
                        ),
                    )
                    .with_help(
                        "record the release tag, version identifier or commit SHA the bytes were \
                         actually taken from; if that is genuinely unknown, omit `pinned_ref` and \
                         say so in `notes` rather than writing a moving pointer",
                    ),
                );
            }
            Some("") => {
                report.push(self.finding(
                    Severity::Warning,
                    codes::UNPINNED,
                    index,
                    "pinned_ref",
                    "`pinned_ref` is blank: no upstream revision is recorded for these bytes",
                ));
            }
            Some(_) => {}
            None => {
                report.push(
                    self.finding(
                        Severity::Warning,
                        codes::UNPINNED,
                        index,
                        "pinned_ref",
                        "no `pinned_ref`: nothing records which upstream revision these bytes \
                         were taken from",
                    )
                    .with_help("record a tag, version identifier or commit SHA once one is known"),
                );
            }
        }
    }

    /// An unknown is allowed. An unexplained unknown is not.
    fn check_gaps(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        let gaps = entry.provenance_gaps();

        for gap in gaps.iter().filter(|gap| **gap != "pinned_ref") {
            report.push(self.finding(
                Severity::Warning,
                codes::PROVENANCE_GAP,
                index,
                gap,
                format!("`{gap}` is not recorded for `{}`", entry.id),
            ));
        }

        if !gaps.is_empty() && entry.notes.as_deref().is_none_or(|n| n.trim().is_empty()) {
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

    /// The facts about bytes in this repository, which are never unknowable.
    fn check_custody(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        if !entry.vendored_path.trim().is_empty() && !is_repo_relative(&entry.vendored_path) {
            report.push(
                self.finding(
                    Severity::Error,
                    codes::PATH_NOT_REPO_RELATIVE,
                    index,
                    "vendored_path",
                    format!(
                        "`vendored_path = \"{}\"` is absolute or climbs out of the registry's \
                         directory, so only one machine can resolve it",
                        entry.vendored_path
                    ),
                )
                .with_help("vendor the artefact into this repository and record the path to it"),
            );
        }

        let sha = entry.sha256.trim();
        if !sha.is_empty() && !is_sha256_hex(sha) {
            report.push(self.finding(
                Severity::Error,
                codes::MALFORMED_SHA256,
                index,
                "sha256",
                format!("`sha256 = \"{sha}\"` is not 64 lower-case hex digits"),
            ));
        }

        let fetched = entry.fetched_at.trim();
        if !fetched.is_empty() && !is_iso_date(fetched) {
            report.push(self.finding(
                Severity::Error,
                codes::MALFORMED_DATE,
                index,
                "fetched_at",
                format!("`fetched_at = \"{fetched}\"` is not an ISO-8601 `YYYY-MM-DD` date"),
            ));
        }
    }

    fn check_urls(&self, index: usize, entry: &SpecEntry, report: &mut ConformanceReport) {
        let poll_endpoint = entry.poll.as_ref().map(|poll| poll.endpoint.clone());
        let fields = [
            ("homepage", entry.homepage.as_deref()),
            ("repository", entry.repository.as_deref()),
            ("poll.endpoint", poll_endpoint.as_deref()),
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

    /// Every diagnostic this crate raises about the registry file goes through
    /// here, so all of them carry a line, a pointer and a spec reference.
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
}

/// The registry's own rules, as a [`Validator`] like any other in this family.
///
/// Implementing the trait is not ceremony: it means a command that already
/// knows how to run validators over documents can run this one over
/// `specs.toml` without learning anything new about registries.
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

/// Whether a string is 64 lower-case hex digits.
///
/// Lower-case specifically: a hash is compared as text in this crate, and two
/// spellings of the same digest that compare unequal would be a drift report
/// nobody can act on.
fn is_sha256_hex(candidate: &str) -> bool {
    candidate.len() == 64
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether a string is an ISO-8601 calendar date, `YYYY-MM-DD`.
///
/// Shape only — it does not know that February has 28 days. The failure this
/// guards against is a free-text date like "August", not an off-by-one on a
/// leap year.
fn is_iso_date(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, byte)| match i {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}
