//! The site generator.
//!
//! Thin on purpose, like `conform-cli`'s binary: everything worth testing
//! lives in the library, where a test can render the whole page in-process and
//! assert on exactly the bytes a reader will see.
//!
//! ```text
//! conform-web [--root <dir>] [--out <dir>] [--wasm <dir>]
//! ```
//!
//! Both default to what the house build script passes, so the common
//! invocation from the repository root is bare.

use std::path::PathBuf;
use std::process::ExitCode;

/// Where the repository root is, when nobody says.
const DEFAULT_ROOT: &str = ".";

/// Where the page is written, when nobody says. Mirrors the house precedent in
/// `roteiro/website/`, whose generator writes into `website/dist`.
const DEFAULT_OUT: &str = "website/dist";

fn main() -> ExitCode {
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut out = PathBuf::from(DEFAULT_OUT);
    // `None` means "alongside the page", which is where `tools/wasm/build.sh`
    // puts it and where the page will look for it. Explicit only so that a
    // caller building the module somewhere else can say so.
    let mut wasm: Option<PathBuf> = None;

    let mut args = std::env::args_os().skip(1);
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--root") => match args.next() {
                Some(value) => root = PathBuf::from(value),
                None => return usage("--root needs a directory"),
            },
            Some("--out") => match args.next() {
                Some(value) => out = PathBuf::from(value),
                None => return usage("--out needs a directory"),
            },
            Some("--wasm") => match args.next() {
                Some(value) => wasm = Some(PathBuf::from(value)),
                None => return usage("--wasm needs a directory"),
            },
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => {
                return usage(&format!(
                    "unexpected argument {}",
                    argument.to_string_lossy()
                ));
            }
        }
    }

    let wasm = wasm.unwrap_or_else(|| out.join(conform_web::WASM_DIR));
    match conform_web::build_with_module(&root, &out, &wasm) {
        Ok(page) => {
            println!("conform-web: wrote {}", page.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("conform-web: {error}");
            ExitCode::FAILURE
        }
    }
}

/// What to print when the command line is not understood.
const USAGE: &str = "conform-web [--root <dir>] [--out <dir>] [--wasm <dir>]\n\n\
    Generates the Open Spec Conform site from `<root>/specs.toml` and the\n\
    manifests under `<root>/crates`, writing `<out>/index.html`.\n\n\
    --root <dir>  the repository root (default: .)\n\
    --out <dir>   where to write the page (default: website/dist)\n\
    --wasm <dir>  where to look for the WebAssembly module\n\
                  (default: <out>/wasm, which is where tools/wasm/build.sh\n\
                  writes it). A missing module is not an error: the page says\n\
                  so, and says which files it went looking for.";

/// Complain, and exit the way a tool with an unusable command line should.
fn usage(problem: &str) -> ExitCode {
    eprintln!("conform-web: {problem}\n\n{USAGE}");
    // 2, matching `conform-cli`'s `EXIT_UNUSABLE`: "no verdict was reached"
    // must not share a code with a verdict.
    ExitCode::from(2)
}
