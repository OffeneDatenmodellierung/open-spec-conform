//! Every crate in this workspace ships both licence texts inside its own
//! published tarball.
//!
//! # The defect this exists to prevent
//!
//! Every member of this workspace declares `license = "MIT OR Apache-2.0"`,
//! inherited from `[workspace.package]`. That declaration is metadata: it tells
//! crates.io what the terms are, and it puts a badge on a web page. It does not
//! put a single byte of licence *text* anywhere.
//!
//! The text lives in `LICENSE-MIT` and `LICENSE-APACHE` at the repository root,
//! and a crate tarball contains only files from that crate's own directory.
//! Without something under `crates/<name>/`, every crate in this family would
//! publish declaring a dual licence while shipping neither licence — which is
//! the one packaging defect that is a licensing problem rather than a build
//! problem, because it is the downloaded tarball, not this repository, that a
//! consumer redistributes.
//!
//! # Why this is a test and not a convention
//!
//! A convention is a thing somebody remembers. A crate added to this workspace
//! next year is added by somebody who has not read this file, and the failure
//! mode is silent: `cargo publish` succeeds, the badge is green, and nothing
//! anywhere says the tarball is empty of terms. So the rule is enforced from
//! the same place the family list is read — this crate already enumerates every
//! member manifest to build the site's crate table, and a member it can see is
//! a member this test can check.
//!
//! # The two halves, and why neither is sufficient
//!
//! [`every_crate_packages_both_licences`] asks cargo what would actually go
//! into the tarball. That is the question that matters, and it is the only one
//! that catches an `exclude` key, a `.gitignore` line, or any future change in
//! how cargo decides what to pack.
//!
//! [`every_crate_licence_is_the_repository_licence`] reads the bytes and
//! compares them to the repository's own. That catches what the listing cannot:
//! a file that is *present under the right name* but is not the licence — a
//! stale copy that has drifted from the root text, or, on a checkout where the
//! filesystem does not support symbolic links, a twenty-byte text file
//! containing the path `../../LICENSE-MIT`. Packaging that would ship a tarball
//! whose `LICENSE-MIT` says nothing at all, and the listing check would call it
//! green.
//!
//! # Why symbolic links
//!
//! `crates/<name>/LICENSE-MIT` is a symbolic link to the repository root's copy,
//! not a duplicate of it. Cargo dereferences it when packaging: the `.crate`
//! archive contains a regular file with the full text, which is what a consumer
//! needs, while this repository keeps exactly one copy of each licence, which is
//! what stops eighteen copies drifting apart one `git apply` at a time.
//!
//! That is a claim about cargo's behaviour, so the first test below is what
//! checks it rather than this comment. And because the byte comparison below
//! does not care *how* the file got there, replacing the links with real copies
//! later would leave both tests passing — the rule is about the bytes, not about
//! the mechanism.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use conform_web::manifests;

/// The two files every crate must carry, named exactly as the repository root
/// names them.
const LICENCES: [&str; 2] = ["LICENSE-MIT", "LICENSE-APACHE"];

/// The repository root — the directory the licence texts live in.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/conform-web should sit two levels below the repository root")
        .to_path_buf()
}

/// Every member of this workspace, by package name and directory.
///
/// Read from the crates directory rather than listed here, for the reason
/// [`manifests`] gives for reading it: a list written in a test is correct until
/// the next crate is added, and then it is a rule that silently stops applying
/// to the very crate nobody has checked yet.
fn members() -> Vec<(String, PathBuf)> {
    let crates_dir = repository_root().join("crates");
    let entries =
        manifests::read_dir(&crates_dir).expect("the workspace's crates directory should read");

    assert!(
        entries
            .iter()
            .any(|entry| entry.name == env!("CARGO_PKG_NAME")),
        "the member list should contain this crate; it found {:?}, which means it read the \
         wrong directory and every assertion below would pass vacuously",
        entries.iter().map(|e| &e.name).collect::<Vec<_>>(),
    );

    entries
        .into_iter()
        .map(|entry| {
            let directory = crates_dir.join(&entry.name);
            (entry.name, directory)
        })
        .collect()
}

/// The names in a `cargo package --list` listing that are missing from it.
///
/// A free function rather than an assertion inline, so that
/// [`the_listing_check_fires_on_a_listing_without_a_licence`] can hand it a
/// listing it built itself and require it to object. A check that is only ever
/// called with real input is a check nobody has seen fail.
fn licences_missing_from(listing: &str) -> Vec<&'static str> {
    let packaged: Vec<&str> = listing.lines().map(str::trim).collect();
    LICENCES
        .into_iter()
        .filter(|licence| !packaged.contains(licence))
        .collect()
}

/// What `cargo package --list` says would go into this crate's tarball.
///
/// `--offline` and `--locked` because this asks a question about files, not
/// about the registry: `Cargo.lock` is committed, so listing a package needs no
/// network, and a test that reaches for one fails for reasons that have nothing
/// to do with the rule it is checking.
///
/// `--allow-dirty` because the tree is dirty every time somebody runs this test
/// while editing, and refusing to answer then would make the check useful only
/// in CI — which is the run where a licence has already been committed missing.
fn packaged_files(package: &str) -> String {
    let output = Command::new(env!("CARGO"))
        .current_dir(repository_root())
        .args([
            "package",
            "--list",
            "--offline",
            "--locked",
            "--allow-dirty",
        ])
        .args(["--package", package])
        .output()
        .unwrap_or_else(|error| panic!("could not run cargo to list {package}: {error}"));

    assert!(
        output.status.success(),
        "`cargo package --list --package {package}` failed ({}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("cargo's listing for {package} was not UTF-8: {error}"))
}

#[test]
fn every_crate_packages_both_licences() {
    let mut faults = Vec::new();

    for (name, _) in members() {
        let listing = packaged_files(&name);

        // The listing is real output, not an empty string that would make the
        // filter below find nothing and report success. Every crate in this
        // workspace has a `readme = "README.md"`, so a listing without one is a
        // listing this test has misread rather than a crate with a problem.
        assert!(
            listing.lines().any(|line| line.trim() == "README.md"),
            "the listing for {name} does not contain README.md, so it is not the listing this \
             test thinks it is:\n{listing}",
        );

        for licence in licences_missing_from(&listing) {
            faults.push(format!(
                "{name} would publish without {licence}: it declares MIT OR Apache-2.0 and ships \
                 no copy of those terms",
            ));
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn every_crate_licence_is_the_repository_licence() {
    let root = repository_root();
    let mut faults = Vec::new();

    for licence in LICENCES {
        let canonical = fs::read(root.join(licence)).unwrap_or_else(|error| {
            panic!("the repository's own {licence} does not read: {error}")
        });

        // A licence this short is not a licence. Both files are kilobytes of
        // terms; anything near the length of the path `../../LICENSE-APACHE` is
        // a symbolic link that a checkout turned into a text file, and this
        // assertion is what stops that reading as agreement below.
        assert!(
            canonical.len() > 512,
            "the repository's own {licence} is {} bytes, which is too short to be a licence",
            canonical.len(),
        );

        for (name, directory) in members() {
            let path = directory.join(licence);
            match fs::read(&path) {
                Err(error) => faults.push(format!("{name}: {licence} does not read: {error}")),
                Ok(bytes) if bytes != canonical => faults.push(format!(
                    "{name}: {licence} is {} bytes and the repository's is {}; the two have \
                     drifted, or this one is an unresolved symbolic link",
                    bytes.len(),
                    canonical.len(),
                )),
                Ok(_) => {}
            }
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn the_listing_check_fires_on_a_listing_without_a_licence() {
    // The listing a crate in this workspace produced before the licences were
    // put where cargo could pack them. If the check below does not object to
    // this, it would not have objected to the real thing either.
    let without = "Cargo.toml\nCargo.toml.orig\nREADME.md\nsrc/lib.rs\n";
    assert_eq!(
        licences_missing_from(without),
        vec!["LICENSE-MIT", "LICENSE-APACHE"],
        "the check should name both licences as missing from a listing that has neither",
    );

    // One present and one absent is the state a half-finished fix leaves
    // behind, and is the one a check that tested for "any licence" would miss.
    let half = "Cargo.toml\nLICENSE-MIT\nREADME.md\nsrc/lib.rs\n";
    assert_eq!(
        licences_missing_from(half),
        vec!["LICENSE-APACHE"],
        "the check should name the one licence missing from a listing that has the other",
    );

    let both = "Cargo.toml\nLICENSE-APACHE\nLICENSE-MIT\nREADME.md\nsrc/lib.rs\n";
    assert!(
        licences_missing_from(both).is_empty(),
        "the check should be satisfied by a listing that has both",
    );
}
