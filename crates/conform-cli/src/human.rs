//! The human-readable rendering: one [`Run`], as plain text.
//!
//! # It computes nothing
//!
//! Every number here is read from the [`Run`]; none is counted a second time.
//! That is not tidiness, it is what makes
//! `tests/json_changes_serialisation_only.rs` pass by construction rather than
//! by vigilance — the two renderers cannot disagree about what was found,
//! because neither of them decides it.
//!
//! # It emits no ANSI at all
//!
//! No colour, anywhere, on purpose. Severity is carried by a word and a glyph,
//! which is what plan §4.2 asks for on accessibility grounds — no
//! colour-only encoding — and it makes `NO_COLOR` honoured by having nothing
//! to honour.
//!
//! The security consequence is the one worth stating plainly: this renderer
//! never writes an escape byte of its own, so any `ESC` on this stream could
//! only have come from a document. [`crate::escape`] is what guarantees none
//! does. Every value below that came from a document, a bundle or the registry
//! goes through `escape::for_terminal` exactly once, on its way out.
//!
//! # The line format is a contract
//!
//! A diagnostic is one header line, with continuations indented under it:
//!
//! ```text
//!   error ODCS003 at contracts/orders.yaml:14:5 (/servers/0/host)
//!       servers[0].host is required
//!       help: add a `host` key, or remove the empty server entry
//!       spec: odcs@3.1.0
//! ```
//!
//! Two spaces of indent for the header, six for everything under it, and a
//! message can never wrap onto a second line because `escape::for_terminal`
//! turns a newline into `<U+000A>`. So `grep` and the equality test can both
//! read this reliably, and a hostile message cannot forge a second finding.

use std::fmt::Write as _;
use std::io::{self, Write};

use conform_core::{Diagnostic, GatePolicy, GateVerdict, Location, Severity};

use crate::escape::for_terminal;
use crate::model::{COMMAND_LINE, Command, DocumentOutcome, Run, SpecSummary};

/// What this binary calls itself, and which version it is.
pub const TOOL_NAME: &str = "conform";
/// This crate's version, as the `--json` envelope and the header report it.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Render a run as plain text.
///
/// # Errors
///
/// Whatever the underlying writer returns.
pub fn render(run: &Run, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "{TOOL_NAME} {TOOL_VERSION} — {}", run.command.as_str())?;
    writeln!(out, "registry: {}", for_terminal(&run.registry_path))?;

    if run.command == Command::RegistryList {
        catalogue(run, out)?;
    } else {
        specs_in_brief(run, out)?;
    }

    findings(run, out)?;
    verdict(run, out)
}

/// The catalogue in full — `registry list`'s whole reason for existing.
///
/// Absent fields are printed as `(not recorded)`, never as blank. A blank
/// looks like a rendering accident; the words are the registry's actual
/// answer, and the difference between "nobody wrote the licence down" and
/// "this renderer dropped a line" is the difference the whole catalogue is for.
fn catalogue(run: &Run, out: &mut dyn Write) -> io::Result<()> {
    for spec in &run.specs {
        writeln!(out)?;
        writeln!(
            out,
            "{} {}  {}",
            spec.verify.glyph(),
            for_terminal(&spec.id),
            for_terminal(&spec.name)
        )?;
        field(out, "version", spec.version.as_deref())?;
        field(out, "pinned_ref", spec.pinned_ref.as_deref())?;
        field(out, "homepage", spec.homepage.as_deref())?;
        field(out, "repository", spec.repository.as_deref())?;
        field(out, "steward", spec.steward.as_deref())?;
        field(out, "licence", spec.licence.as_deref())?;
        writeln!(
            out,
            "    vendored     {}",
            for_terminal(&spec.vendored_path)
        )?;
        writeln!(
            out,
            "    sha256       {}  (re-hashed just now: {})",
            for_terminal(&spec.sha256),
            spec.verify.as_str()
        )?;
        field(out, "fetched_at", Some(&spec.fetched_at))?;
        if !spec.provenance_gaps.is_empty() {
            writeln!(out, "    gaps         {}", spec.provenance_gaps.join(", "))?;
        }
        writeln!(
            out,
            "    validator    {}",
            if spec.has_adapter {
                "yes — `conform validate --spec` can check documents against this"
            } else {
                "no — catalogued and verified, with no validator in this binary"
            }
        )?;
    }
    Ok(())
}

/// One provenance field, present or honestly absent.
fn field(out: &mut dyn Write, name: &str, value: Option<&str>) -> io::Result<()> {
    match value {
        Some(value) => writeln!(out, "    {name:<12} {}", for_terminal(value)),
        None => writeln!(out, "    {name:<12} (not recorded)"),
    }
}

/// The specifications in scope, one line each, with the pin and the drift
/// status — the same two facts the TUI's spec pane keeps on screen.
fn specs_in_brief(run: &Run, out: &mut dyn Write) -> io::Result<()> {
    if run.specs.is_empty() {
        return Ok(());
    }
    writeln!(out)?;
    writeln!(out, "specs")?;
    for spec in &run.specs {
        writeln!(
            out,
            "  {} {:<6} {:<8} bytes {}",
            spec.verify.glyph(),
            for_terminal(&spec.id),
            for_terminal(spec.version.as_deref().unwrap_or("—")),
            spec.verify.as_str(),
        )?;
        writeln!(
            out,
            "      pinned   {}",
            for_terminal(spec.pinned_ref.as_deref().unwrap_or("(not recorded)"))
        )?;
        writeln!(out, "      upstream {}", upstream(spec))?;
    }
    Ok(())
}

/// Everything found, grouped by the document it was found in.
fn findings(run: &Run, out: &mut dyn Write) -> io::Result<()> {
    for document in &run.documents {
        if document.report.is_empty() {
            continue;
        }
        writeln!(out)?;
        writeln!(
            out,
            "{} {}{}",
            document.glyph(),
            for_terminal(&document.id),
            match &document.spec_id {
                Some(id) => format!("  ({})", for_terminal(id)),
                None => String::new(),
            }
        )?;
        for diagnostic in &document.report {
            finding(diagnostic, out)?;
        }
    }
    Ok(())
}

/// One diagnostic, in the format this module's documentation specifies.
fn finding(diagnostic: &Diagnostic, out: &mut dyn Write) -> io::Result<()> {
    writeln!(
        out,
        "  {} {} at {}",
        diagnostic.severity,
        for_terminal(diagnostic.code.as_str()),
        location(&diagnostic.location)
    )?;
    writeln!(out, "      {}", for_terminal(&diagnostic.message))?;
    if let Some(help) = &diagnostic.help {
        writeln!(out, "      help: {}", for_terminal(help))?;
    }
    if let Some(spec_ref) = &diagnostic.spec_ref {
        writeln!(out, "      spec: {}", for_terminal(&spec_ref.to_string()))?;
    }
    Ok(())
}

/// A location, escaped component by component.
///
/// Deliberately not `Location`'s own `Display`: that would interpolate a raw
/// document name straight into the line, and a document name is something a
/// hostile tree gets to choose. Line and column are integers and cannot carry
/// anything; the two strings are escaped.
#[must_use]
pub fn location(location: &Location) -> String {
    let mut rendered = for_terminal(location.document.as_str()).into_owned();
    if let Some(line) = location.line {
        let _ = write!(rendered, ":{line}");
        if let Some(column) = location.column {
            let _ = write!(rendered, ":{column}");
        }
    }
    if let Some(pointer) = &location.pointer {
        let _ = write!(rendered, " ({})", for_terminal(pointer.as_str()));
    }
    rendered
}

/// The tally, and then — separately — what the policy made of it.
///
/// Two paragraphs on purpose. The first says what was found; the second says
/// whether that fails anything. They are different questions, `conform-core`
/// models them as different questions, and printing them as one line is how
/// they stop being different questions.
fn verdict(run: &Run, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "found: {} error(s), {} warning(s), {} info across {} document(s)",
        run.count(Severity::Error),
        run.count(Severity::Warning),
        run.count(Severity::Info),
        examined(&run.documents),
    )?;

    match run.policy {
        GatePolicy::Never => writeln!(
            out,
            "gate:  none — reporting only, so this run does not fail on anything it found \
             (pass --gate errors to gate)"
        )?,
        GatePolicy::AtOrAbove(threshold) => match run.verdict() {
            GateVerdict::Gated { worst } => writeln!(
                out,
                "gate:  at or above {threshold} — GATED, worst severity found was {worst}"
            )?,
            GateVerdict::Passed { worst } => writeln!(
                out,
                "gate:  at or above {threshold} — passed, worst severity found was {}",
                worst.map_or("nothing", Severity::as_str)
            )?,
        },
    }

    if run.unusable {
        writeln!(
            out,
            "exit:  2 — the run could not be performed, so nothing here is a verdict on any \
             document"
        )?;
    } else {
        writeln!(out, "exit:  {}", run.exit_code())?;
    }
    Ok(())
}

/// How many documents were actually examined.
///
/// The synthetic outcomes — `<command line>`, and the registry file itself —
/// are findings about the invocation, not documents that were checked, and
/// counting them as documents would overstate what was done.
fn examined(documents: &[DocumentOutcome]) -> usize {
    documents
        .iter()
        .filter(|document| document.id != COMMAND_LINE)
        .count()
}

/// The upstream link for a specification, as the human renderer spells it.
///
/// Shared with the TUI so the two cannot disagree about what "the upstream
/// link" means for an entry that records a repository and no homepage.
#[must_use]
pub fn upstream(spec: &SpecSummary) -> String {
    for_terminal(spec.upstream_link().unwrap_or("(not recorded)")).into_owned()
}
