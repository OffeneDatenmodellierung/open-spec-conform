//! The registry and the vendored schemas, frozen into the artefact.
//!
//! What [`crate::engine`] does by reading a `specs.toml` off disk, this module
//! does by reading one the compiler put inside the binary. It exists because
//! the [`wasm`](crate::wasm) binding has to run somewhere with no filesystem,
//! and everything else here follows from refusing to let that fact cost
//! anything.
//!
//! # Why the bytes are embedded and not taken from the caller
//!
//! The obvious alternative — a constructor that takes schema bytes from
//! JavaScript — is cheaper to write and is wrong. `conform_validator_new` is
//! documented as the place "where the registry is read, where the digest is
//! verified, and where a drifted schema is refused"; a bytes-in constructor
//! moves that decision to the caller. The browser would then validate against
//! whatever bytes the page happened to load, chosen by the page, with this
//! repository's SHA-256 check reduced to decoration. The vendored artefact
//! this whole registry was built in response to was called
//! `odps-json-schema-latest.json`, and a bytes-in constructor is that file
//! reincarnated in a different runtime.
//!
//! So nothing is trusted from the host. The registry text and the schema text
//! are both `include_str!`, the digest gate still runs, and a caller names a
//! spec id — never a schema.
//!
//! # What the embedded check catches, and what it cannot
//!
//! It catches a registry edited without re-hashing, a schema edited without
//! updating the registry, and a re-vendoring that moved the artefact out from
//! under the `include_str!` — see [`Embedded::vendored_path`]. Both sides are
//! frozen into the same artefact, so it **cannot** catch drift on disk after
//! the build; that remains `conform registry verify`'s job in CI, and saying
//! otherwise would be claiming a guarantee this module does not have.
//!
//! # No `unsafe`, no pointers, no feature-dependent behaviour
//!
//! Ordinary safe Rust, held to the crate's `deny(unsafe_code)` like everything
//! outside [`crate::abi`]. The `wasm` feature gates it — the default build
//! carries no embedded schema bytes and is the artefact it always was — but
//! the feature, not the architecture: with `--all-features` this module is
//! compiled and its tests run on the *host*, so the digest gate is exercised
//! by `cargo test` and read by `cargo clippy` rather than left to be
//! discovered by a browser.

use conform_core::SpecRef;
use conform_odcs::OdcsValidator;
use conform_odps::OdpsValidator;
use conform_registry::{Registry, SpecEntry, sha256_hex};

use crate::engine::{Backend, BuildError, render};

/// The registry, as text, exactly as it is on disk at the repository root.
///
/// `include_str!` rather than a transcription, for the reason every other
/// crate in this family reads `specs.toml` rather than restating it: a fact
/// about a specification that is written down twice is a fact that can be
/// wrong in one of the two places.
const REGISTRY_TOML: &str = include_str!("../../../specs.toml");

/// What this module names the registry in a diagnostic.
///
/// The path a reader would type, not the one the compiler resolved: the
/// `../../../` in the `include_str!` above is an artefact of where this file
/// lives and means nothing to somebody reading an error message.
const REGISTRY_NAME: &str = "specs.toml";

/// One vendored schema, and the registry entry it must still answer to.
#[derive(Debug, Clone, Copy)]
pub struct Embedded {
    /// The registry identifier this schema is the schema for.
    pub spec_id: &'static str,
    /// Where the registry says these bytes live, repository-relative.
    ///
    /// Written out because `include_str!` takes a literal and a literal cannot
    /// be read from the registry — which makes this the one fact in this
    /// module that *is* transcribed, and therefore the one that has to be
    /// checked. [`build`] compares it against the entry's own
    /// `vendored_path` and refuses a mismatch, so a re-vendoring that changes
    /// the filename fails the build of a validator rather than silently
    /// validating against the previous version's bytes.
    pub vendored_path: &'static str,
    /// The schema itself.
    pub schema: &'static str,
}

/// Every schema frozen into this artefact.
///
/// The same two standards [`crate::engine::REACHABLE_SPEC_IDS`] reaches, and
/// for the same reason: `okf` is a directory of cross-referencing files, and
/// a boundary that takes one document's bytes has nothing to hand it.
pub const EMBEDDED: &[Embedded] = &[
    Embedded {
        spec_id: "odcs",
        vendored_path: "schemas/odcs-json-schema-v3.1.0.json",
        schema: include_str!("../../../schemas/odcs-json-schema-v3.1.0.json"),
    },
    Embedded {
        spec_id: "odps",
        vendored_path: "schemas/odps-json-schema-v1.0.0.json",
        schema: include_str!("../../../schemas/odps-json-schema-v1.0.0.json"),
    },
];

/// The embedded record for a spec id, if this artefact carries one.
#[must_use]
pub fn find(spec_id: &str) -> Option<&'static Embedded> {
    EMBEDDED.iter().find(|entry| entry.spec_id == spec_id)
}

/// The spec ids this artefact can build a validator for without a filesystem.
#[must_use]
pub fn spec_ids() -> Vec<&'static str> {
    EMBEDDED.iter().map(|entry| entry.spec_id).collect()
}

/// Build a validator for `spec_id` from the embedded registry and schema.
///
/// The order is the order [`OdcsValidator::from_registry`] uses, and it is the
/// order that matters: find the entry, **re-hash the bytes against the digest
/// the registry records**, and only then compile. Step two is the whole point
/// of this module existing rather than an `include_str!` handed straight to a
/// schema compiler.
///
/// # Errors
///
/// [`BuildError::UnknownSpec`] if nothing is embedded for that id.
/// [`BuildError::Registry`] if the embedded registry will not parse, holds no
/// such entry, records a different `vendored_path` than the bytes that were
/// embedded, records a digest the embedded bytes do not hash to, or names a
/// schema that will not compile.
pub fn build(spec_id: &str) -> Result<Backend, BuildError> {
    let Some(embedded) = find(spec_id) else {
        return Err(BuildError::UnknownSpec(format!(
            "`{spec_id}` names no schema embedded in this build; it carries {}",
            spec_ids().join(" and ")
        )));
    };

    let registry = Registry::load_str(REGISTRY_TOML, REGISTRY_NAME)
        .map_err(|error| BuildError::Registry(render(&error.into_report())))?;

    let Some(entry) = registry.find(spec_id) else {
        return Err(BuildError::Registry(format!(
            "the embedded registry holds no `{spec_id}` entry, so the schema embedded alongside \
             it has no digest to be checked against"
        )));
    };

    // The transcribed fact, checked. See `Embedded::vendored_path`.
    if entry.vendored_path != embedded.vendored_path {
        return Err(BuildError::Registry(format!(
            "the embedded `{spec_id}` schema is stale: `{REGISTRY_NAME}` now points at \
             `{}`, and the bytes compiled into this build came from `{}`",
            entry.vendored_path, embedded.vendored_path
        )));
    }

    // The gate. Nothing below this line runs against bytes that did not hash
    // to what the registry records for them.
    let digest = sha256_hex(embedded.schema.as_bytes());
    if !digest.eq_ignore_ascii_case(entry.sha256.trim()) {
        return Err(BuildError::Registry(format!(
            "refusing to validate against the embedded `{spec_id}` schema: the bytes compiled \
             into this build hash to {digest}, and `{REGISTRY_NAME}` records {} for \
             `{}`",
            entry.sha256.trim(),
            entry.vendored_path
        )));
    }

    compile(embedded, entry)
}

/// Compile the verified bytes into the adapter that speaks for them.
///
/// Reached only after the digest matched; split out so that the gate above
/// reads as a gate rather than as one arm of a match.
fn compile(embedded: &Embedded, entry: &SpecEntry) -> Result<Backend, BuildError> {
    let mut spec = SpecRef::new(entry.id.clone());
    if let Some(version) = &entry.version {
        spec = spec.with_version(version.clone());
    }

    let provenance = provenance(entry);
    let name = embedded.vendored_path;

    match embedded.spec_id {
        "odcs" => OdcsValidator::from_schema_str(embedded.schema, spec, provenance, name)
            .map(|validator| Backend::Odcs(Box::new(validator)))
            .map_err(|error| BuildError::Registry(render(error.report()))),
        "odps" => OdpsValidator::from_schema_str(embedded.schema, spec, provenance, name)
            .map(|validator| Backend::Odps(Box::new(validator)))
            .map_err(|error| BuildError::Registry(render(error.report()))),
        // Unreachable: `EMBEDDED` is this module's own table and holds exactly
        // the two ids above. Written as a real arm rather than an
        // `unreachable!`, for the reason `engine::Backend::build` gives for
        // doing the same — a panic in the crate whose subject is panics would
        // be a poor joke, and on the target this module exists for it would
        // take the instance down.
        other => Err(BuildError::UnknownSpec(format!(
            "`{other}` is embedded in this build but nothing here knows how to compile it"
        ))),
    }
}

/// The sentence every report will carry under the adapter's
/// `…904 VALIDATED_AGAINST` code.
///
/// It says *embedded*, and it says what that does not cover. A provenance
/// sentence that read the same as the on-disk one would be claiming a check
/// this module cannot perform — see the note at the top of this file.
fn provenance(entry: &SpecEntry) -> String {
    match &entry.pinned_ref {
        Some(pinned) => format!(
            "bytes of `{}` embedded in this build at compile time and re-hashed here against the \
             digest `{REGISTRY_NAME}` records for upstream pin `{pinned}`; the registry is \
             embedded alongside them, so this check cannot speak for the files on disk today",
            entry.vendored_path
        ),
        None => format!(
            "bytes of `{}` embedded in this build at compile time and re-hashed here against the \
             digest `{REGISTRY_NAME}` records; the entry records no upstream pin, and the \
             registry is embedded alongside the bytes, so this check cannot speak for the files \
             on disk today",
            entry.vendored_path
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_schema_matches_the_digest_the_registry_records() {
        for embedded in EMBEDDED {
            let backend = build(embedded.spec_id).unwrap_or_else(|error| {
                panic!("`{}` did not build: {error}", embedded.spec_id);
            });
            assert_eq!(backend.spec().id, embedded.spec_id);
        }
    }

    #[test]
    fn the_registry_and_the_embedded_bytes_agree_on_where_they_came_from() {
        let registry = Registry::load_str(REGISTRY_TOML, REGISTRY_NAME).expect("embedded registry");
        for embedded in EMBEDDED {
            let entry = registry
                .find(embedded.spec_id)
                .unwrap_or_else(|| panic!("no `{}` entry", embedded.spec_id));
            assert_eq!(entry.vendored_path, embedded.vendored_path);
            assert_eq!(
                sha256_hex(embedded.schema.as_bytes()),
                entry.sha256.trim().to_ascii_lowercase(),
            );
        }
    }

    #[test]
    fn an_id_with_nothing_embedded_for_it_is_refused_by_name() {
        let error = build("okf").expect_err("`okf` is not embedded");
        assert!(matches!(error, BuildError::UnknownSpec(_)), "{error:?}");
        assert!(error.message().contains("odcs"), "{error}");
        assert!(error.message().contains("odps"), "{error}");
    }

    #[test]
    fn the_provenance_sentence_says_what_the_check_cannot_cover() {
        let registry = Registry::load_str(REGISTRY_TOML, REGISTRY_NAME).expect("embedded registry");
        let entry = registry.find("odcs").expect("odcs entry");
        let sentence = provenance(entry);
        assert!(sentence.contains("embedded"), "{sentence}");
        assert!(sentence.contains("on disk today"), "{sentence}");
    }
}
