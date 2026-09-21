//! Generate `crates/conform-ffi/include/conform.h`.
//!
//! One job, deliberately: read `conform-ffi`'s source with the configuration
//! in its `cbindgen.toml`, and write the header. It does not compare, it does
//! not decide whether the result is acceptable, and it does not have an
//! opinion about where it is run from. The comparison lives in
//! `crates/conform-ffi/tests/the_header_matches_the_library.rs`, which has the
//! better failure message and is the thing CI runs.
//!
//! See `Cargo.toml` in this directory for why the generator is outside the
//! workspace rather than a dependency of the crate it generates for.
//!
//! ```sh
//! # write the committed header
//! cargo run --manifest-path tools/headergen/Cargo.toml
//!
//! # write somewhere else, which is what the drift test does
//! cargo run --manifest-path tools/headergen/Cargo.toml -- /tmp/conform.h
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The crate whose ABI this describes, relative to this manifest.
const FFI_CRATE: &str = "../../crates/conform-ffi";

fn main() -> ExitCode {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FFI_CRATE);
    let destination = match std::env::args_os().nth(1) {
        Some(path) => PathBuf::from(path),
        None => crate_dir.join("include").join("conform.h"),
    };

    match generate(&crate_dir) {
        Ok(header) => match std::fs::write(&destination, header) {
            Ok(()) => {
                println!("wrote {}", destination.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("could not write {}: {error}", destination.display());
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Render the header, or say why not.
fn generate(crate_dir: &Path) -> Result<String, String> {
    let config_path = crate_dir.join("cbindgen.toml");
    let config = cbindgen::Config::from_file(&config_path)
        .map_err(|error| format!("{}: {error}", config_path.display()))?;

    let bindings = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .map_err(|error| format!("{}: {error}", crate_dir.display()))?;

    let mut rendered = Vec::new();
    bindings.write(&mut rendered);
    String::from_utf8(rendered).map_err(|error| format!("cbindgen emitted non-UTF-8: {error}"))
}
