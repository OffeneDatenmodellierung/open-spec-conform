//! What this binary carries when nothing on disk answers.
//!
//! # The three things that can quietly go wrong
//!
//! **The embedded catalogue can fall behind the repository's.** The bytes are
//! reached through symbolic links, so they cannot drift on their own — but an
//! *eighth* specification added to `specs.toml` without a link beside it would
//! leave an installed binary reporting a catalogue entry whose artefact it does
//! not carry. [`every_registry_entry_has_its_bytes_embedded`] is what stops
//! that reaching a release.
//!
//! **The embedded bytes can stop being the repository's bytes.** A checkout
//! whose filesystem does not support symbolic links turns each link into a
//! short text file holding its own path, and `include_str!` would then embed
//! that path as if it were a schema. [`the_embedded_bytes_are_the_repositorys_bytes`]
//! compares them.
//!
//! **The digest gate can become decoration.** This is the one that matters. An
//! embedded schema that is compiled rather than checked is a *worse* false
//! green than an unverified file on disk, because the reader cannot go and look
//! at the file to see for themselves — there is no file.
//! [`embedded_bytes_that_do_not_match_the_digest_are_refused`] plants bytes that
//! do not hash to the recorded digest and requires a refusal.
//!
//! # Why this reads the registry rather than listing what it expects
//!
//! A list here would be a third spelling of the catalogue, after `specs.toml`
//! and `embedded::ARTEFACTS`, and the one nobody updates. Everything below is
//! derived from the embedded registry itself.

use conform_cli::embedded;
use conform_core::Severity;
use conform_registry::{Registry, sha256_hex, verify_bytes};

/// The catalogue this binary carries, parsed.
fn embedded_registry() -> Registry {
    Registry::load_str(embedded::REGISTRY_TOML, embedded::REGISTRY_NAME)
        .expect("the embedded `specs.toml` should parse; it is the repository's own")
}

#[test]
fn every_registry_entry_has_its_bytes_embedded() {
    let registry = embedded_registry();

    // Non-vacuity: a registry that parsed to nothing would make every loop
    // below pass without examining anything.
    assert!(
        registry.entries().len() >= 7,
        "the embedded registry holds {} entries, which is fewer than this repository vendors — \
         the wrong bytes were embedded",
        registry.entries().len(),
    );

    let mut missing = Vec::new();
    for entry in registry.entries() {
        if embedded::artefact(&entry.vendored_path).is_none() {
            missing.push(format!(
                "`{}` records `vendored_path = \"{}\"` and no bytes are embedded for it; add a \
                 symbolic link under `crates/conform-cli/embedded/` at that same path and a line \
                 to `embedded::ARTEFACTS`",
                entry.id, entry.vendored_path,
            ));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn nothing_is_embedded_that_the_registry_does_not_record() {
    let registry = embedded_registry();
    let recorded: Vec<&str> = registry
        .entries()
        .iter()
        .map(|entry| entry.vendored_path.as_str())
        .collect();

    // The other direction. Bytes carried for an artefact the catalogue no
    // longer records are weight in every installed binary that nothing can
    // ever verify, because there is no digest to check them against.
    let mut orphans = Vec::new();
    for (path, _) in embedded::ARTEFACTS {
        if !recorded.contains(path) {
            orphans.push(format!(
                "`{path}` is embedded and the registry records no entry with that \
                 `vendored_path`; it cannot be verified against anything",
            ));
        }
    }
    assert!(orphans.is_empty(), "{}", orphans.join("\n"));
}

#[test]
fn the_embedded_bytes_are_the_repositorys_bytes() {
    let registry = embedded_registry();
    let mut faults = Vec::new();

    for (index, entry) in registry.entries().iter().enumerate() {
        let Some(text) = embedded::artefact(&entry.vendored_path) else {
            continue; // reported by `every_registry_entry_has_its_bytes_embedded`
        };
        let diagnostic = verify_bytes(index, entry, text.as_bytes(), "embedded");
        if diagnostic.severity >= Severity::Error {
            faults.push(format!(
                "`{}`: the embedded bytes hash to {} and the embedded registry records {} — the \
                 symbolic link was replaced by a copy that has drifted, or this checkout did not \
                 resolve it",
                entry.id,
                sha256_hex(text.as_bytes()),
                entry.sha256.trim(),
            ));
        }
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn embedded_bytes_that_do_not_match_the_digest_are_refused() {
    let registry = embedded_registry();
    let entry = registry
        .find("odcs")
        .expect("the embedded registry records `odcs`");
    let index = registry
        .entries()
        .iter()
        .position(|e| e.id == "odcs")
        .expect("`odcs` has an index");

    let real = embedded::artefact(&entry.vendored_path).expect("`odcs` bytes are embedded");

    // The same length, deliberately: a check that compared sizes rather than
    // digests would pass this, and would pass a schema edited to say something
    // different in the same number of bytes.
    let planted = real.replacen("\"title\"", "\"titlz\"", 1);
    assert_eq!(
        planted.len(),
        real.len(),
        "the planted bytes should differ in content and not in length, so that only a real \
         digest comparison can tell them apart",
    );
    assert_ne!(planted, real, "the plant should actually change something");

    let verdict = verify_bytes(index, entry, planted.as_bytes(), "embedded");
    assert!(
        verdict.severity >= Severity::Error,
        "bytes that do not hash to the recorded digest must be refused, and this returned \
         {:?}: {}",
        verdict.severity,
        verdict.message,
    );
    assert_eq!(
        verdict.code.as_str(),
        conform_registry::codes::SHA256_MISMATCH,
        "the refusal should carry the registry's own drift code, so a reader gets `REG022` from \
         the embedded path exactly as they would from the on-disk one",
    );

    // And the real bytes still pass, so the assertion above is about the plant
    // rather than about the checker refusing everything.
    let control = verify_bytes(index, entry, real.as_bytes(), "embedded");
    assert!(
        control.severity < Severity::Error,
        "the genuine embedded bytes should pass the same check: {}",
        control.message,
    );
}
