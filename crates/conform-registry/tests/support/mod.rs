//! Locating the registry every test in this crate is about.

// Each integration test compiles this module separately, so a helper used by
// one of them is dead code in the others. The alternative — a helper per test
// file — would let the tests drift apart on how they load the registry, which
// is the one thing they must agree on.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use conform_registry::Registry;

/// The workspace root — the directory `specs.toml` lives in.
pub fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-registry should sit two levels below the workspace root")
        .to_path_buf()
}

/// The real registry, loaded exactly the way a consumer would load it.
pub fn real_registry() -> Registry {
    let path = workspace_root().join("specs.toml");
    Registry::load_path(&path)
        .unwrap_or_else(|error| panic!("{} did not load: {error}", path.display()))
}

/// The real registry's text, for tests that mutate it to prove a rule bites.
pub fn real_registry_text() -> String {
    let path = workspace_root().join("specs.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}
