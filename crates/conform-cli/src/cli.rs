//! The command surface.
//!
//! `clap` lives in this module and nowhere else. Everything below it takes a
//! [`Request`], which is a plain record — so the engine, the renderers and
//! every test can be driven without constructing a command line, and a change
//! to the argument syntax cannot reach into the part of the program that does
//! the work.
//!
//! # Gating is opt-in, and that is the point
//!
//! `conform validate` over a document with twelve errors prints twelve
//! findings and **exits 0**. That is not a bug and it is not leniency: it is
//! the report-versus-gate separation `conform-core` is built around, arriving
//! at the one place a user meets it. Reporting says what is true about a
//! document. Gating says what should fail a build, and it is a decision the
//! caller makes with `--gate` (or `--check`, its `errors` shorthand), never
//! one this binary makes on their behalf.
//!
//! Collapsing the two is how a warning ends up failing somebody's unrelated
//! pull request, and a gate that cries wolf is a gate that gets switched off.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use conform_core::{GatePolicy, Severity};

use crate::engine::Request;
use crate::model::Command;

/// `conform` — navigate and verify the specifications this repository
/// conforms to.
#[derive(Debug, Parser)]
#[command(
    name = "conform",
    version,
    about = "Navigate and verify the specifications this repository conforms to.",
    long_about = "Navigate and verify the specifications this repository conforms to.\n\n\
                  Reporting and gating are separate: `conform validate` prints everything it \
                  finds and exits 0. Pass --gate (or --check) to make findings fail the run.",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Option<Action>,

    /// Options that mean the same thing wherever they appear.
    #[command(flatten)]
    pub common: Common,
}

/// The options every subcommand accepts.
#[derive(Debug, Args)]
pub struct Common {
    /// Emit the machine-readable JSON envelope instead of a human report.
    ///
    /// Serialisation only: the same findings, the same exit code, a different
    /// spelling.
    #[arg(long, global = true)]
    pub json: bool,

    /// Restrict everything to one registry entry, by id — `odcs`, `odps`,
    /// `okf`, `odcl`.
    #[arg(long, global = true, value_name = "ID")]
    pub spec: Option<String>,

    /// What fails the run. Default: nothing.
    #[arg(long, global = true, value_name = "POLICY", conflicts_with = "check")]
    pub gate: Option<GateArg>,

    /// Shorthand for `--gate errors`.
    #[arg(long, global = true)]
    pub check: bool,

    /// Where `specs.toml` is. Default: the nearest one at or above the current
    /// directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub registry: Option<PathBuf>,
}

impl Common {
    /// The gating policy these options describe.
    #[must_use]
    pub fn policy(&self) -> GatePolicy {
        if self.check {
            return GatePolicy::errors_only();
        }
        match self.gate {
            None | Some(GateArg::Never) => GatePolicy::report_only(),
            Some(GateArg::Errors) => GatePolicy::AtOrAbove(Severity::Error),
            Some(GateArg::Warnings) => GatePolicy::AtOrAbove(Severity::Warning),
        }
    }
}

/// What `--gate` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GateArg {
    /// Report everything, fail on nothing. The default.
    Never,
    /// Fail if anything of error severity was found.
    Errors,
    /// Fail on warnings as well as errors.
    Warnings,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Action {
    /// Check documents against the standard each is written in.
    ///
    /// A file is routed by its `kind` key — `DataContract` to ODCS,
    /// `DataProduct` to ODPS — and a directory holding an `index.md` is read
    /// as an OKF bundle. `--spec` overrides the routing.
    Validate {
        /// Files or directories to check.
        #[arg(value_name = "PATH", required = true)]
        paths: Vec<PathBuf>,
    },

    /// Open the interactive console.
    ///
    /// The same thing bare `conform` does. Named as well, so it can be asked
    /// for explicitly in a script or a desktop entry.
    Tui {
        /// Files or directories to load into it. Optional: with none, the
        /// console opens on the catalogue and what re-hashing the vendored
        /// bytes just said.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
    },

    /// The specification catalogue.
    Registry {
        /// Which question to ask of it.
        #[command(subcommand)]
        question: RegistryAction,
    },
}

/// The registry subcommands, which are the registry's own two questions.
#[derive(Debug, Subcommand)]
pub enum RegistryAction {
    /// List every catalogued specification, with its upstream provenance, and
    /// report what the registry's own rules say about the record.
    List,
    /// Re-hash every vendored artefact against the digest `specs.toml` records
    /// for it.
    Verify,
}

impl Cli {
    /// Turn a parsed command line into the request the engine takes.
    ///
    /// `None` when there is no work to describe — no subcommand was given.
    #[must_use]
    pub fn request(&self) -> Option<Request> {
        let (command, paths) = match self.command.as_ref()? {
            // The console shows what the non-interactive commands show, so it
            // asks the engine the same question — which is what keeps it a
            // view. With no paths there is nothing to validate, and the
            // catalogue's own integrity is the useful thing to open on.
            Action::Tui { paths } if paths.is_empty() => (Command::RegistryVerify, Vec::new()),
            Action::Validate { paths } | Action::Tui { paths } => {
                (Command::Validate, paths.clone())
            }
            Action::Registry { question } => (
                match question {
                    RegistryAction::List => Command::RegistryList,
                    RegistryAction::Verify => Command::RegistryVerify,
                },
                Vec::new(),
            ),
        };

        Some(Request {
            command,
            paths,
            spec: self.common.spec.clone(),
            policy: self.common.policy(),
            registry: self.common.registry.clone(),
        })
    }
}

impl Cli {
    /// Whether this invocation opens the console.
    ///
    /// Bare `conform` does, because a console is what a person at a terminal
    /// with no arguments almost certainly wanted. Every other form is
    /// non-interactive, so nothing in a script can ever find itself waiting
    /// for a keystroke.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        matches!(self.command, None | Some(Action::Tui { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn the_command_surface_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_bare_validate_gates_on_nothing() {
        let cli = Cli::try_parse_from(["conform", "validate", "x.yaml"]).expect("parses");
        assert_eq!(cli.common.policy(), GatePolicy::Never);
    }

    #[test]
    fn gating_is_opt_in_and_spelled_two_ways() {
        for args in [
            vec!["conform", "validate", "x.yaml", "--gate", "errors"],
            vec!["conform", "validate", "x.yaml", "--check"],
        ] {
            let cli = Cli::try_parse_from(args).expect("parses");
            assert_eq!(cli.common.policy(), GatePolicy::AtOrAbove(Severity::Error));
        }

        let cli = Cli::try_parse_from(["conform", "validate", "x.yaml", "--gate", "warnings"])
            .expect("parses");
        assert_eq!(
            cli.common.policy(),
            GatePolicy::AtOrAbove(Severity::Warning)
        );
    }

    #[test]
    fn the_two_spellings_of_gating_may_not_be_combined() {
        // They would have to be reconciled, and a silent winner between two
        // explicit instructions is how a run gates differently from how its
        // author read it.
        assert!(
            Cli::try_parse_from([
                "conform", "validate", "x.yaml", "--check", "--gate", "never"
            ])
            .is_err()
        );
    }

    #[test]
    fn json_and_spec_are_accepted_before_or_after_the_subcommand() {
        for args in [
            vec!["conform", "--json", "--spec", "odcs", "registry", "verify"],
            vec!["conform", "registry", "verify", "--json", "--spec", "odcs"],
        ] {
            let cli = Cli::try_parse_from(args).expect("parses");
            assert!(cli.common.json);
            assert_eq!(cli.common.spec.as_deref(), Some("odcs"));
            let request = cli.request().expect("a subcommand was given");
            assert_eq!(request.command, Command::RegistryVerify);
        }
    }

    #[test]
    fn validate_insists_on_being_told_what_to_check() {
        // Defaulting to the current directory would walk whatever happens to
        // be below it, which is not a thing to do by accident.
        assert!(Cli::try_parse_from(["conform", "validate"]).is_err());
    }
}
