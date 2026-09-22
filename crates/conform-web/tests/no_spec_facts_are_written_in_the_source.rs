//! No specification fact is written into this crate's source.
//!
//! # The rule
//!
//! A hand-typed upstream link is the drift this whole project exists to
//! prevent: it is correct on the day somebody types it and silently wrong
//! afterwards, with nothing to notice. The registry is the single source of
//! truth, so the site must contain no URL, digest, pin, version or name of a
//! specification as a literal — it must read every one of them.
//!
//! # Why this test is not the only one, and not the strongest
//!
//! Scanning source for a string is a *negative* check, and negative checks
//! fail quietly in two ways: the scanner can look in the wrong place, and the
//! fact list can be empty. Both are guarded below —
//! [`the_scanner_fires_on_a_planted_url`] plants a real URL in a string and
//! requires the scanner to find it, and
//! [`the_fact_list_is_not_empty`] requires the registry to have supplied
//! enough facts to be worth scanning for.
//!
//! The *positive* proof lives next door in
//! `the_page_is_a_function_of_the_registry.rs`, which renders the page from a
//! fabricated registry and asserts the real facts do not appear. This file
//! catches a literal sitting in a comment or a stylesheet, which that one
//! would not; that one catches a fact obtained by any route at all, which this
//! one would not. Neither subsumes the other.

use std::fs;
use std::path::{Path, PathBuf};

use conform_registry::Registry;

/// The files the rule applies to: everything a human writes in this crate.
///
/// The generated page is deliberately *not* scanned — it is supposed to be
/// full of registry facts, that being the point of generating it.
fn source_files() -> Vec<PathBuf> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for directory in ["src", "tests"] {
        collect(&crate_root.join(directory), &mut files);
    }
    files.push(crate_root.join("Cargo.toml"));

    // The build script that drives the generator lives with the site rather
    // than with the crate, and a URL typed into it would be just as wrong.
    let build = repository_root().join("website").join("build.sh");
    if build.is_file() {
        files.push(build);
    }
    files
}

/// Every file under a directory, recursively.
fn collect(directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    for entry in listing.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, into);
        } else if path.is_file() {
            into.push(path);
        }
    }
}

/// One fact from the registry, and where it came from, so a failure names the
/// entry a reader has to go and look at.
struct Fact {
    spec: String,
    field: &'static str,
    value: String,
}

/// Every specification fact a reader could be tempted to transcribe.
///
/// Read from the registry rather than listed here — a list here would be the
/// very transcription this test forbids, and would go stale in exactly the
/// same way.
fn facts() -> Vec<Fact> {
    let registry =
        Registry::load_path(repository_root().join("specs.toml")).expect("the registry reads");

    let mut facts = Vec::new();
    for entry in registry.entries() {
        let mut push = |field: &'static str, value: Option<&str>| {
            if let Some(value) = value {
                let value = value.trim();
                // Short values are excluded on purpose, and the exclusion is
                // the honest part of this test. `MIT`, `1.0` and `Bitol` are
                // real registry values and are also substrings of ordinary
                // English and of licence headers; requiring them absent would
                // make this test fail for reasons that have nothing to do with
                // transcription. What covers them instead is the substitution
                // test next door, which does not match substrings at all — it
                // renders from a fabricated registry and looks at what comes
                // out.
                if value.len() >= 8 {
                    facts.push(Fact {
                        spec: entry.id.clone(),
                        field,
                        value: value.to_owned(),
                    });
                }
            }
        };

        push("name", Some(&entry.name));
        push("homepage", entry.homepage.as_deref());
        push("repository", entry.repository.as_deref());
        push("steward", entry.steward.as_deref());
        for version in &entry.versions {
            push("sha256", Some(&version.sha256));
            push("vendored_path", Some(&version.vendored_path));
            push("version", version.version.as_deref());
            push("pinned_ref", version.pinned_ref.as_deref());
            if let Some(poll) = &version.poll {
                push("poll.endpoint", Some(&poll.endpoint));
            }
        }
    }
    facts
}

#[test]
fn the_fact_list_is_not_empty() {
    // The first way a scan can pass for nothing: having nothing to scan for.
    let facts = facts();
    assert!(
        facts.len() >= 20,
        "only {} registry facts were collected; a scan over that few proves little",
        facts.len(),
    );

    // And it must contain URLs specifically, since those are the drift the
    // rule is actually about.
    let urls = facts
        .iter()
        .filter(|fact| fact.value.starts_with("https://"))
        .count();
    assert!(
        urls >= 5,
        "only {urls} of the collected facts are URLs; the rule this test enforces is about URLs",
    );
}

#[test]
fn the_file_list_is_not_empty() {
    // The second way a scan can pass for nothing: looking in the wrong place.
    let files = source_files();
    assert!(
        files.len() >= 5,
        "only {} source files were found; the scanner is looking in the wrong directory",
        files.len(),
    );
    assert!(
        files.iter().any(|f| f.ends_with("render.rs")),
        "the renderer is the file most likely to hold a hand-typed link, and it was not scanned",
    );
}

#[test]
fn the_scanner_fires_on_a_planted_url() {
    // The control, and the thing that makes the scan below non-vacuous: a real
    // registry URL, planted in a string, must be found. If this fails, every
    // "no facts found" result in this file is worthless.
    let facts = facts();
    let planted = facts
        .iter()
        .find(|fact| fact.value.starts_with("https://"))
        .expect("the registry records at least one URL");

    let haystack = format!("// a hand-typed link: {}\n", planted.value);
    let hits = scan(&haystack, &facts);

    assert!(
        hits.iter().any(|hit| hit.contains(&planted.value)),
        "the scanner did not find {:?} in text that plainly contains it",
        planted.value,
    );
}

#[test]
fn the_scanner_does_not_fire_on_text_without_facts() {
    // The other half of the control. A scanner that reports a hit on
    // everything would also pass the test above.
    let clean = "// this comment contains no registry fact at all\n\
                 let url = entry.homepage.as_deref();\n";
    assert!(
        scan(clean, &facts()).is_empty(),
        "the scanner reported a fact in text that has none, so its findings mean nothing",
    );
}

#[test]
fn no_source_file_contains_a_specification_fact() {
    let facts = facts();
    let mut findings = Vec::new();

    for path in source_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for hit in scan(&text, &facts) {
            findings.push(format!("{}: {hit}", display(&path)));
        }
    }

    assert!(
        findings.is_empty(),
        "specification facts are written into this crate's source. Read them from the \
         registry instead — a transcription is correct today and silently wrong tomorrow, \
         which is the whole reason `specs.toml` exists:\n  {}",
        findings.join("\n  "),
    );
}

/// Every registry fact that appears literally in some text.
fn scan(text: &str, facts: &[Fact]) -> Vec<String> {
    facts
        .iter()
        .filter(|fact| text.contains(&fact.value))
        .map(|fact| {
            format!(
                "`{}`'s {} appears literally: {:?}",
                fact.spec, fact.field, fact.value,
            )
        })
        .collect()
}

/// A path as a reader would name it: relative to the repository root.
fn display(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The repository root, found relative to this crate.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/conform-web is two levels below the root")
        .to_path_buf()
}
