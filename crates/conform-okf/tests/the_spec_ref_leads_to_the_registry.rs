//! A `SpecRef` that leads nowhere is decoration.
//!
//! Every diagnostic this crate raises carries `SpecRef { id: "okf", version:
//! "0.2" }`, and the whole reason `conform-core` puts a reference on a
//! diagnostic is so a reader can answer "which version of which standard said
//! this was wrong, and where did those bytes come from?". That answer lives in
//! `specs.toml`, and nothing else in this workspace would notice if the two
//! drifted apart: `conform-registry` does not know this crate exists, and this
//! crate does not read the registry at run time.
//!
//! So the link is asserted here, once, in the only place that can see both
//! ends of it.

use std::path::{Path, PathBuf};

use conform_registry::Registry;

fn repository_root() -> PathBuf {
    // `crates/conform-okf` → the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is two directories below the repository root")
        .to_path_buf()
}

fn registry() -> Registry {
    Registry::load_path(repository_root().join("specs.toml")).expect("load specs.toml")
}

#[test]
fn the_spec_this_crate_stamps_on_every_diagnostic_is_a_registry_entry() {
    let registry = registry();
    let entry = registry
        .find(conform_okf::SPEC_ID)
        .unwrap_or_else(|| panic!("specs.toml has no `{}` entry", conform_okf::SPEC_ID));

    assert_eq!(
        entry.version.as_deref(),
        Some(conform_okf::OKF_VERSION),
        "this crate implements a version of the specification the registry does not describe"
    );
    assert!(
        entry.is_pinned(),
        "the `okf` entry records no immutable upstream revision, so nothing can say which \
         revision of the corpus these rules were written against"
    );
}

/// And the artefact that entry pins is the manifest beside this crate's own
/// fixtures — not some other file that happens to hash correctly.
#[test]
fn the_entry_pins_this_crate_s_fixture_manifest() {
    let registry = registry();
    let entry = registry.find(conform_okf::SPEC_ID).expect("the okf entry");

    let recorded = repository_root().join(&entry.vendored_path);
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/okf-upstream/SHA256SUMS");

    assert_eq!(
        recorded, manifest,
        "the registry pins an artefact that is not this crate's fixture manifest"
    );
    assert!(manifest.is_file(), "the pinned manifest is not there");
}

/// The registry's own integrity check, run over the entry this crate owns.
///
/// `conform-registry` runs this across every entry in its own test suite. It
/// is repeated here for one entry because this is the crate whose fixtures
/// would be edited, and a failure here names the cause directly instead of
/// arriving as a digest mismatch in a crate that has nothing to do with OKF.
#[test]
fn the_pinned_manifest_still_hashes_to_what_the_registry_records() {
    use conform_core::Severity;

    let registry = registry();
    let (index, entry) = registry
        .entries()
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.id == conform_okf::SPEC_ID)
        .expect("the okf entry");

    let diagnostic = registry.verify_entry(index, entry);
    assert_eq!(
        diagnostic.severity,
        Severity::Info,
        "the pinned manifest no longer matches the registry: {diagnostic}"
    );
    assert_eq!(
        diagnostic.code.as_str(),
        conform_registry::codes::ARTEFACT_VERIFIED
    );
}
