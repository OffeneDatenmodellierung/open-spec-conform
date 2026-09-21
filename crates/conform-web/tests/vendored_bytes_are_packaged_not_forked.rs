//! No crate directory holds a diverging copy of a repository-root artefact.
//!
//! # The rule
//!
//! `specs.toml` and the files under `schemas/` are the artefacts this whole
//! project is about: the registry records a SHA-256 for each of them precisely
//! so that there is one authority for what those bytes are. A second copy with
//! its own future is `odps-json-schema-latest.json` — the vendored file with no
//! version, no source URL and no fetch date that motivated `conform-registry`
//! in the first place — reincarnated one directory over.
//!
//! So: a file under `crates/` that carries the name of a repository-root
//! artefact must carry its bytes too.
//!
//! # Why any crate directory holds one at all
//!
//! Because a `.crate` archive contains no file from above the package root.
//! `conform-ffi`'s `wasm` feature freezes the registry and two schemas into the
//! artefact with `include_str!`, and the obvious spelling of those paths —
//! `../../../specs.toml` — reaches out of the crate and into the repository
//! root. That compiles in this workspace and nowhere else: published, the crate
//! would fail to build for everybody who downloaded it with that feature on.
//!
//! The bytes therefore have to be *inside* `crates/conform-ffi/`, and the
//! question this test settles is which kind of inside. They are symbolic links
//! to the repository's single copy, so there is nothing to diverge; cargo
//! dereferences them when packaging, so the tarball still carries real files.
//! But "they are symbolic links" is a fact about how somebody created them one
//! afternoon, and this test is what makes it a fact about the repository —
//! including on a checkout whose filesystem turned each link into a short text
//! file containing its own path.
//!
//! # What this does not cover
//!
//! `crates/conform-okf/tests/fixtures/okf-upstream/` is vendored too, and is
//! deliberately not compared against anything here: it has a registry entry of
//! its own, `vendored_path` points *into* that directory, and
//! `conform-okf`'s own `vendored_fixtures_match_the_manifest` checks it against
//! the digest recorded for it. This test is about the root's artefacts being
//! copied, not about vendoring in general.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use conform_web::manifests;

/// The repository root.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-web should sit two levels below the repository root")
        .to_path_buf()
}

/// Every artefact the repository root is the authority for, by file name.
///
/// Read from the filesystem rather than listed here, so that a schema vendored
/// next year is covered by this rule on the day it lands rather than on the day
/// somebody remembers to add it.
fn authorities() -> BTreeMap<String, Vec<u8>> {
    let root = repository_root();
    let mut authorities = BTreeMap::new();

    let registry = root.join("specs.toml");
    authorities.insert(
        "specs.toml".to_owned(),
        fs::read(&registry).expect("the repository's own specs.toml should read"),
    );

    let schemas = root.join("schemas");
    for entry in fs::read_dir(&schemas).expect("the repository's schemas directory should read") {
        let path = entry.expect("a schemas directory entry should read").path();
        if path.is_file() {
            let name = path
                .file_name()
                .expect("a file has a name")
                .to_string_lossy()
                .into_owned();
            authorities.insert(
                name,
                fs::read(&path).expect("a vendored schema should read"),
            );
        }
    }

    assert!(
        authorities.len() > 1,
        "only {} root artefact(s) found, which means this test looked in the wrong place and \
         every comparison below would be vacuous",
        authorities.len(),
    );
    authorities
}

/// Every file under a directory, recursively, skipping `target`.
fn walk(directory: &Path, into: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            walk(&path, into)?;
        } else if path.is_file() {
            into.push(path);
        }
    }
    Ok(())
}

/// Every file in every crate of this workspace.
fn files_under_every_crate() -> Vec<PathBuf> {
    let crates_dir = repository_root().join("crates");
    let members =
        manifests::read_dir(&crates_dir).expect("the workspace's crates directory should read");
    assert!(
        members
            .iter()
            .any(|member| member.name == env!("CARGO_PKG_NAME")),
        "the member list should contain this crate; without it this test read the wrong directory",
    );

    let mut files = Vec::new();
    for member in &members {
        walk(&crates_dir.join(&member.name), &mut files)
            .unwrap_or_else(|error| panic!("cannot walk {}: {error}", member.name));
    }
    files
}

#[test]
fn a_crate_that_carries_a_root_artefact_carries_its_bytes() {
    let authorities = authorities();
    let root = repository_root();

    let mut compared = Vec::new();
    let mut faults = Vec::new();

    for path in files_under_every_crate() {
        let name = path
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let Some(canonical) = authorities.get(&name) else {
            continue;
        };

        let relative = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();

        // `okf-upstream` is vendored against its own registry entry and is
        // checked by `conform-okf`. See the note at the top of this file.
        if relative.contains("okf-upstream") {
            continue;
        }

        compared.push(relative.clone());
        match fs::read(&path) {
            Err(error) => faults.push(format!("{relative}: does not read: {error}")),
            Ok(bytes) if &bytes != canonical => faults.push(format!(
                "{relative} is {} bytes and the repository's {name} is {}: this is a second copy \
                 of a vendored artefact with its own future, which is the defect `specs.toml` \
                 exists to make impossible — or it is a symbolic link this checkout did not \
                 resolve, in which case what gets published is the link's own path",
                bytes.len(),
                canonical.len(),
            )),
            Ok(_) => {}
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));

    // Non-vacuity. `conform-ffi` needs three of these inside its own directory
    // to be publishable at all, so a run that compared nothing is a run that
    // found nothing to compare — which would mean this test had stopped
    // watching the very thing it was written for.
    assert!(
        !compared.is_empty(),
        "no crate directory carries a copy of a root artefact, so this test compared nothing; \
         either the scan is broken or `conform-ffi` has lost the bytes its `wasm` feature \
         embeds",
    );
}

#[test]
fn the_comparison_fires_on_bytes_that_differ() {
    // The state a checkout without symbolic-link support leaves behind: a file
    // under the right name holding the path it was supposed to point at. If
    // the comparison above did not object to this, it would not have objected
    // to a hand-edited fork of a schema either.
    let canonical = authorities()
        .remove("specs.toml")
        .expect("specs.toml is an authority");
    let unresolved_link = b"../../../specs.toml".to_vec();

    assert_ne!(
        unresolved_link, canonical,
        "an unresolved symbolic link should not compare equal to the registry it names",
    );
    assert!(
        canonical.len() > unresolved_link.len(),
        "the registry should be substantially longer than a path to it; if it is not, the \
         comparison above is reading something that is not the registry",
    );
}
