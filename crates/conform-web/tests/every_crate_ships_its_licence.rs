//! Every crate in this workspace ships, inside its own published tarball, the
//! text of every licence the tarball is redistributed under.
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
//! # The third file, and the five crates that owe it
//!
//! `LICENSE-MIT` and `LICENSE-APACHE` are this workspace's own offer, and they
//! say nothing about material this workspace did not write. Five crates here
//! redistribute material taken from `data-modelling-sdk`, which is MIT under a
//! *different* notice: its copyright line reads "Copyright (c) 2025 Mark
//! Olliver", where this repository's own `LICENSE-MIT` reads "Copyright (c)
//! 2026 Mark Olliver and the open-spec-conform contributors". MIT attaches
//! exactly one condition — that its notice travels with every copy and
//! substantial portion — and a notice naming a different holder and a different
//! year is not preserved by shipping ours in its place. So the upstream one is
//! kept verbatim at `LICENSE-MIT-upstream`, and every crate that carries the
//! material it covers has to carry it too.
//!
//! Four of the five are the `conform-model-*` crates, each a port of a module
//! from that repository's `crates/core/src/models/`. The fifth is
//! `conform-cli`, which ships the vendored schemas alongside its embedded
//! catalogue — and one of those schemas is `data-modelling-sdk`'s own work
//! rather than a standards body's, which is why the registry records a licence
//! for it at all: three of its seven entries record none, because nobody
//! upstream wrote one down.
//!
//! # The fourth file, and why a second notice was needed
//!
//! `NOTICE-ossie-upstream` is the same obligation arriving from a different
//! licence. Two of the vendored schemas are Apache-2.0, and Apache-2.0 section
//! 4(d) says that where the work includes a NOTICE file, a redistribution must
//! carry a readable copy of the attribution notices in it. Upstream includes
//! one, and — because the project changed hands between the two revisions
//! vendored here — it is not the same NOTICE at both. Both are quoted verbatim
//! in that file, along with the incubator DISCLAIMER that accompanies the
//! newer of the two.
//!
//! Only `conform-cli` owes it, for the same reason it owes the MIT notice: it
//! is the crate that ships the vendored bytes alongside its embedded
//! catalogue. Which schemas those are, and which revisions they came from, are
//! registry facts and are not transcribed here.
//!
//! Which schema that is, and which commit its bytes were last written at, are
//! registry facts, and this file does not transcribe registry facts — read them
//! from `specs.toml`, for the reason
//! `no_spec_facts_are_written_in_the_source.rs` exists to enforce.
//!
//! Their READMEs already told the reader that the notice was "preserved at
//! `LICENSE-MIT-upstream` in the repository root". That sentence was true of
//! this repository and false of every tarball built from it: a `.crate` archive
//! holds nothing from above the package root, so the published artefact would
//! have cited a file it did not contain, while redistributing the very material
//! that file exists to cover. `cargo publish` cannot be undone, and this rule
//! was written before the first upload rather than after it.
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
//! [`every_crate_packages_the_licences_it_owes`] asks cargo what would actually
//! go into the tarball. That is the question that matters, and it is the only
//! one that catches an `exclude` key, a `.gitignore` line, or any future change
//! in how cargo decides what to pack.
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
//! # What [`REDISTRIBUTORS`] can and cannot catch
//!
//! The list of crates that owe the upstream notice is authored, not derived.
//! The only fact the filesystem offers is whether a crate already carries the
//! file, and a rule read from that would agree with whatever is there — it
//! would call a missing notice correct for exactly as long as it was missing.
//!
//! Being authored, it is checked in both directions. A listed crate that does
//! not package the notice fails, which is the defect above. A crate that
//! packages it without being listed fails too, because a notice for material a
//! crate does not carry is a false claim about that crate's provenance, and the
//! remedy — record why it is there — is the same work either way.
//!
//! What no test here can catch is the third case: a crate that ports upstream
//! material and is added to neither the list nor the tarball. Nothing in the
//! bytes distinguishes a port from original work. That one is caught by reading
//! the diff, and it is written down here so that a green run is not mistaken
//! for a proof that no such crate exists.
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

/// The upstream MIT notice, named exactly as the repository root names it.
///
/// Carried by the crates in [`REDISTRIBUTORS`] and by no others.
const UPSTREAM_LICENCE: &str = "LICENSE-MIT-upstream";

/// The upstream attribution notices for the Apache-2.0 material vendored here,
/// named exactly as the repository root names them.
///
/// Carried by the crates in [`NOTICE_REDISTRIBUTORS`] and by no others.
const UPSTREAM_NOTICE: &str = "NOTICE-ossie-upstream";

/// Every crate that redistributes Apache-2.0 material whose distribution
/// includes a NOTICE file, and what each one redistributes.
///
/// Authored and checked in both directions, on exactly the reasoning
/// [`REDISTRIBUTORS`] is checked in both directions.
const NOTICE_REDISTRIBUTORS: [(&str, &str); 1] = [(
    "conform-cli",
    "embeds, with its catalogue, two vendored schemas taken from an Apache-2.0 project      that distributes a NOTICE file with them",
)];

/// Every crate that redistributes `data-modelling-sdk` material, and what each
/// one redistributes.
///
/// The second field is the reason the crate is on this list. It is stored here
/// rather than left to a README because it is what the failure message needs to
/// say, and because the next person to change this list should not have to
/// reconstruct the argument from four provenance sections.
const REDISTRIBUTORS: [(&str, &str); 5] = [
    (
        "conform-model-cads",
        "ports `crates/core/src/models/cads.rs`",
    ),
    (
        "conform-model-dbmv",
        "ports `crates/core/src/models/dbmv.rs`",
    ),
    ("conform-model-odcs", "ports `crates/core/src/models/odcs/`"),
    (
        "conform-model-odps",
        "ports `crates/core/src/models/odps.rs`",
    ),
    (
        "conform-cli",
        "embeds, with its catalogue, the one vendored schema the registry records \
         as that repository's own work",
    ),
];

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

/// Why this crate owes the upstream MIT notice, or [`None`] if it does not.
fn redistributes_upstream(name: &str) -> Option<&'static str> {
    REDISTRIBUTORS
        .into_iter()
        .find(|(crate_name, _)| *crate_name == name)
        .map(|(_, reason)| reason)
}

/// Why this crate owes the upstream Apache-2.0 notices, or [`None`] if it does
/// not.
fn redistributes_notice(name: &str) -> Option<&'static str> {
    NOTICE_REDISTRIBUTORS
        .into_iter()
        .find(|(crate_name, _)| *crate_name == name)
        .map(|(_, reason)| reason)
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

/// Whether a `cargo package --list` listing carries a named file.
///
/// Separate from [`licences_missing_from`] because the answer is read in both
/// directions: the crates that owe a notice must package it, and every other
/// crate must not.
///
/// The file is a parameter rather than a constant because there are now two
/// such notices — one arriving from MIT, one from Apache-2.0 — and two
/// near-identical matchers would be two places for the exact-match rule the
/// unit test below pins to drift apart.
fn packages(listing: &str, file: &str) -> bool {
    listing.lines().map(str::trim).any(|line| line == file)
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
fn every_crate_packages_the_licences_it_owes() {
    let members = members();
    let mut faults = Vec::new();

    // A name in either list that is not a member is a typo, and a typo there
    // is a crate whose obligation silently stops being checked — the same
    // vacuous pass the member list is read from disk to avoid.
    for (list, (name, _)) in REDISTRIBUTORS
        .map(|entry| ("REDISTRIBUTORS", entry))
        .into_iter()
        .chain(NOTICE_REDISTRIBUTORS.map(|entry| ("NOTICE_REDISTRIBUTORS", entry)))
    {
        assert!(
            members.iter().any(|(member, _)| member == name),
            "{list} names {name}, which is not a member of this workspace; the rule it \
             states would never be checked against anything",
        );
    }

    for (name, _) in &members {
        let listing = packaged_files(name);

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

        match (
            redistributes_upstream(name),
            packages(&listing, UPSTREAM_LICENCE),
        ) {
            (Some(reason), false) => faults.push(format!(
                "{name} would publish without {UPSTREAM_LICENCE}: it {reason} from \
                 data-modelling-sdk, whose MIT notice names a different copyright holder and year \
                 than this workspace's own LICENSE-MIT, so shipping ours does not preserve it",
            )),
            (None, true) => faults.push(format!(
                "{name} packages {UPSTREAM_LICENCE} but is not listed in REDISTRIBUTORS; either \
                 record there what it redistributes from data-modelling-sdk, or remove the file, \
                 because a notice for material a crate does not carry is a false claim about that \
                 crate's provenance",
            )),
            (Some(_), true) | (None, false) => {}
        }

        match (
            redistributes_notice(name),
            packages(&listing, UPSTREAM_NOTICE),
        ) {
            (Some(reason), false) => faults.push(format!(
                "{name} would publish without {UPSTREAM_NOTICE}: it {reason}, and Apache-2.0 \
                 section 4(d) requires those attribution notices to travel with the bytes rather \
                 than stay behind in this repository",
            )),
            (None, true) => faults.push(format!(
                "{name} packages {UPSTREAM_NOTICE} but is not listed in NOTICE_REDISTRIBUTORS; \
                 either record there what it redistributes, or remove the file, because a notice \
                 for material a crate does not carry is a false claim about that crate's \
                 provenance",
            )),
            (Some(_), true) | (None, false) => {}
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn every_crate_licence_is_the_repository_licence() {
    let root = repository_root();
    let members = members();
    let mut faults = Vec::new();

    // A licence this short is not a licence. Every one of these files is
    // kilobytes of terms; anything near the length of the path
    // `../../LICENSE-APACHE` is a symbolic link that a checkout turned into a
    // text file, and this closure is what stops that reading as agreement below.
    let canonical = |licence: &str| {
        let bytes = fs::read(root.join(licence)).unwrap_or_else(|error| {
            panic!("the repository's own {licence} does not read: {error}")
        });
        assert!(
            bytes.len() > 512,
            "the repository's own {licence} is {} bytes, which is too short to be a licence",
            bytes.len(),
        );
        bytes
    };

    let mut compare =
        |name: &str, path: PathBuf, licence: &str, canonical: &[u8]| match fs::read(&path) {
            Err(error) => faults.push(format!("{name}: {licence} does not read: {error}")),
            Ok(bytes) if bytes != canonical => faults.push(format!(
                "{name}: {licence} is {} bytes and the repository's is {}; the two have \
                 drifted, or this one is an unresolved symbolic link",
                bytes.len(),
                canonical.len(),
            )),
            Ok(_) => {}
        };

    for licence in LICENCES {
        let text = canonical(licence);
        for (name, directory) in &members {
            compare(name, directory.join(licence), licence, &text);
        }
    }

    // The upstream notice is held to the same standard, and for a sharper
    // reason: it is the only file here whose bytes a third party's licence
    // requires, so a drifted or unresolved copy is a condition unmet rather than
    // a promise of our own broken.
    let upstream = canonical(UPSTREAM_LICENCE);
    for (name, _) in REDISTRIBUTORS {
        let directory = root.join("crates").join(name);
        compare(
            name,
            directory.join(UPSTREAM_LICENCE),
            UPSTREAM_LICENCE,
            &upstream,
        );
    }

    let notice = canonical(UPSTREAM_NOTICE);
    for (name, _) in NOTICE_REDISTRIBUTORS {
        let directory = root.join("crates").join(name);
        compare(
            name,
            directory.join(UPSTREAM_NOTICE),
            UPSTREAM_NOTICE,
            &notice,
        );
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

#[test]
fn the_upstream_notice_check_reads_a_listing_in_both_directions() {
    // Exactly the listing `cargo package --list -p conform-model-cads` produced
    // before this rule existed: both of the workspace's own licences present,
    // which is what made the defect invisible, and the upstream notice absent.
    let before = "Cargo.toml\nLICENSE-APACHE\nLICENSE-MIT\nREADME.md\nsrc/lib.rs\n";
    assert!(
        !packages(before, UPSTREAM_LICENCE),
        "the check should not find the upstream notice in a listing that has only the \
         workspace's own two licences",
    );

    let after = "Cargo.toml\nLICENSE-APACHE\nLICENSE-MIT\nLICENSE-MIT-upstream\nREADME.md\n";
    assert!(
        packages(after, UPSTREAM_LICENCE),
        "the check should find the upstream notice in a listing that has it",
    );

    // `LICENSE-MIT` is a prefix of `LICENSE-MIT-upstream`, so a check written
    // with `contains` or `starts_with` would read the first as the second and
    // call every crate in the workspace compliant.
    assert!(
        !packages("Cargo.toml\nLICENSE-MIT\nREADME.md\n", UPSTREAM_LICENCE),
        "the check should not mistake LICENSE-MIT for LICENSE-MIT-upstream",
    );

    // And a crate directory holding a file whose name merely starts the same
    // way is not the notice either.
    assert!(
        !packages(
            "Cargo.toml\nLICENSE-MIT-upstream.md\nREADME.md\n",
            UPSTREAM_LICENCE
        ),
        "the check should match the notice's name exactly, not as a prefix",
    );

    // The Apache-2.0 notice is read by the same matcher, and the two must not
    // stand in for one another: a crate carrying the MIT notice and owing the
    // Apache one would otherwise read as compliant.
    let mit_only = "Cargo.toml\nLICENSE-MIT-upstream\nREADME.md\n";
    assert!(
        !packages(mit_only, UPSTREAM_NOTICE),
        "the check should not accept the MIT notice in place of the Apache-2.0 one",
    );
    assert!(
        packages(
            "Cargo.toml\nNOTICE-ossie-upstream\nREADME.md\n",
            UPSTREAM_NOTICE
        ),
        "the check should find the Apache-2.0 notice in a listing that has it",
    );
    assert!(
        !packages("Cargo.toml\nNOTICE\nREADME.md\n", UPSTREAM_NOTICE),
        "the check should match the notice's name exactly, not as a prefix",
    );
}

#[test]
fn every_crate_that_owes_the_upstream_notice_says_why() {
    // A list entry with an empty reason is a list entry whose failure message
    // would read "it  from data-modelling-sdk", and the reason is the whole
    // value of recording the crate rather than just its name.
    for (name, reason) in REDISTRIBUTORS.into_iter().chain(NOTICE_REDISTRIBUTORS) {
        assert!(
            !reason.trim().is_empty(),
            "{name} is listed as redistributing upstream material with no reason recorded",
        );
    }
}
