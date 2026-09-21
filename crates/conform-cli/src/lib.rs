//! `conform` — the console for the specifications this repository conforms to.
//!
//! This crate is the binary's whole implementation. The binary itself is four
//! lines, so that everything here can be driven from a test with no process,
//! no terminal and no temporary files.
//!
//! # One set of facts, three renderings
//!
//! ```text
//!                 ┌─ human ──► stdout, terminal-escaped
//!  Request ─► Run ├─ json  ──► stdout, JSON-encoded, NOT terminal-escaped
//!                 └─ tui   ──► a terminal, terminal-escaped
//! ```
//!
//! [`engine::run`] does the work once and returns a [`model::Run`]. The three
//! renderers read it and nothing else: none of them validates a document, none
//! recounts a diagnostic, and none decides whether the run gates. They
//! therefore cannot disagree about what was found, which is what
//! `tests/json_changes_serialisation_only.rs` holds them to — and the TUI is a
//! *view*, in the strict sense, rather than a second implementation of the
//! same idea.
//!
//! # Reporting and gating are separate decisions
//!
//! `conform validate` over a document full of errors prints every one of them
//! and **exits 0**. Gating is opt-in, with `--gate` or `--check`. See
//! [`model::Run::exit_code`] for the three codes and what each means.
//!
//! # This crate is the renderer, and escaping is its obligation
//!
//! Diagnostic messages quote third-party content verbatim and are deliberately
//! left unescaped by the adapters that produce them, because escaping is not
//! idempotent and must happen exactly once, at the point of display. That
//! point is here. See [`escape`] for what is neutralised and why `--json` is
//! deliberately exempt.
//!
//! # Example
//!
//! ```no_run
//! let mut out = Vec::new();
//! let mut err = Vec::new();
//! let code = conform_cli::execute(["conform", "registry", "verify"], &mut out, &mut err);
//! assert_eq!(code, 0);
//! ```

pub mod cli;
pub mod codes;
pub mod discover;
pub mod embedded;
pub mod engine;
pub mod escape;
pub mod human;
pub mod json;
pub mod model;
pub mod tui;

use std::ffi::OsString;
use std::io::Write;

use clap::Parser as _;

use crate::cli::Cli;

/// The exit code for a command line that could not be understood, and for a
/// run that could not be performed.
///
/// One code for both, because they are the same statement to a caller: *no
/// verdict was reached*. What must never share a code with either is a run
/// that reached a verdict, whatever that verdict was.
pub const EXIT_UNUSABLE: i32 = 2;

/// Run the binary.
///
/// Takes its arguments and its output streams rather than reaching for
/// `std::env` and `std::io::stdout`, so a test can drive a whole invocation
/// in-process and read exactly what a user would have seen.
///
/// Returns the process exit code. See [`model::Run::exit_code`].
pub fn execute<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            // `--help` and `--version` arrive here as "errors" that are not
            // failures: they print to stdout and succeed, which is what every
            // other tool does and what shell pipelines expect.
            let rendered = error.render();
            return if error.use_stderr() {
                let _ = write!(err, "{rendered}");
                EXIT_UNUSABLE
            } else {
                let _ = write!(out, "{rendered}");
                0
            };
        }
    };

    // Bare `conform`, with nothing else to go on, means the console.
    let request = cli.request().unwrap_or_else(|| engine::Request {
        command: model::Command::RegistryVerify,
        paths: Vec::new(),
        spec: cli.common.spec.clone(),
        policy: cli.common.policy(),
        registry: cli.common.registry.clone(),
    });

    let run = engine::run(&request);

    // The console and `--json` are mutually exclusive by construction: one is
    // a terminal and the other is a pipe, and asking for both is asking for
    // the envelope.
    if cli.is_interactive() && !cli.common.json {
        return match tui::run(run, cli.common.spec.as_deref()) {
            Ok(()) => 0,
            Err(error) => {
                let _ = writeln!(err, "conform: the console could not start: {error}");
                EXIT_UNUSABLE
            }
        };
    }

    let rendered = if cli.common.json {
        json::render(&run, out)
    } else {
        human::render(&run, out)
    };

    if let Err(error) = rendered {
        let _ = writeln!(err, "conform: cannot write the report: {error}");
        return EXIT_UNUSABLE;
    }
    run.exit_code()
}
