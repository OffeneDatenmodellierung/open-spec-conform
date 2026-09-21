//! The registry and every artefact it records, frozen into the binary.
//!
//! # Why this exists
//!
//! `conform-cli` is the crate a person installs. `cargo install conform-cli`
//! puts a `conform` on their PATH, and until this module existed that binary
//! could not check anything: it searched for a `specs.toml` at or above the
//! working directory, found none, and exited 2 under `CLI100`. The message was
//! honest and the tool was useless, which is a poor trade for a tool whose
//! entire purpose is verifying specifications.
//!
//! So the catalogue and the bytes it vouches for are compiled in, and are the
//! last resort when nothing on disk answers.
//!
//! # Last resort, and never a silent one
//!
//! [`crate::engine::RegistryOrigin`] holds the precedence: an explicit
//! `--registry` wins, then a `specs.toml` discovered on disk, then this. The
//! order matters more than it looks. Somebody standing in a checkout is asking
//! about *that* checkout, and a binary that answered from its own memory would
//! report the specifications it was built with while the reader believed they
//! were seeing the ones in front of them — a false green of exactly the kind
//! this project exists to prevent.
//!
//! Every renderer therefore names the origin. `registry: embedded in
//! conform-cli 0.1.0` is not decoration; it is the difference between a
//! verdict a reader can act on and one they have to go and check.
//!
//! # These bytes are pinned at publish time
//!
//! A released binary carries the registry as it stood when that version was
//! published, for as long as it stays installed. `conform 0.1.0` holds
//! `0.1.0`'s view of upstream in 2030. That is not a defect — a frozen
//! catalogue is what makes an embedded verdict reproducible — but it is a
//! property a user has to know, so the embedded provenance sentence says the
//! version the bytes came from and every report carries it.
//!
//! Point `--registry` at a checkout to check against something newer.
//!
//! # One authority for the bytes, here as everywhere
//!
//! `crates/conform-cli/embedded/` contains **symbolic links** to the
//! repository's single copy of `specs.toml` and of each vendored artefact,
//! laid out under the same repository-relative paths the registry records —
//! so the key this module looks an artefact up by *is* `vendored_path`, rather
//! than a second spelling of it that could disagree.
//!
//! Cargo dereferences those links when packaging, so the published tarball
//! carries real bytes while this repository keeps one copy of each. A copy
//! here would be a second set of vendored bytes with its own future, which is
//! `odps-json-schema-latest.json` — the artefact `conform-registry` was built
//! in response to — reincarnated one directory over.
//!
//! The table below is the only transcription in this module, and
//! `tests/the_embedded_registry_is_complete_and_honest.rs` is what stops it
//! being a place where the truth can drift: it requires the table and the
//! embedded registry to name exactly the same artefacts, so a specification
//! added to `specs.toml` without being embedded fails the build of this
//! workspace rather than surfacing later as a missing artefact in somebody
//! else's terminal.

/// The catalogue, as text: the repository's own `specs.toml`, reached through
/// the symbolic link in `embedded/`.
///
/// `include_str!` cannot reach outside the crate directory and survive
/// packaging — a `.crate` archive holds no file from above the package root —
/// which is why the path goes through `embedded/` rather than `../../../`.
pub const REGISTRY_TOML: &str = include_str!("../embedded/specs.toml");

/// What the renderers call this registry, and what a diagnostic about it names
/// as its document.
pub const REGISTRY_NAME: &str = "specs.toml (embedded)";

/// Every vendored artefact this binary carries, keyed by the `vendored_path`
/// the registry records for it.
///
/// The key is the registry's own spelling of the path, not a second one: it is
/// what [`artefact`] is asked for and what the completeness test compares
/// against the registry. `include_str!` needs a literal, so the path appears
/// twice on each line — and those two spellings agreeing is precisely what the
/// test checks.
pub const ARTEFACTS: &[(&str, &str)] = &[
    (
        "schemas/odcs-json-schema-v3.1.0.json",
        include_str!("../embedded/schemas/odcs-json-schema-v3.1.0.json"),
    ),
    (
        "schemas/odps-json-schema-v1.0.0.json",
        include_str!("../embedded/schemas/odps-json-schema-v1.0.0.json"),
    ),
    (
        "schemas/odcl-json-schema-1.2.1.json",
        include_str!("../embedded/schemas/odcl-json-schema-1.2.1.json"),
    ),
    (
        "schemas/cads.schema.json",
        include_str!("../embedded/schemas/cads.schema.json"),
    ),
    (
        "crates/conform-okf/tests/fixtures/okf-upstream/SHA256SUMS",
        include_str!("../embedded/crates/conform-okf/tests/fixtures/okf-upstream/SHA256SUMS"),
    ),
];

/// The bytes embedded for a `vendored_path`, if this binary carries them.
///
/// [`None`] is a real answer and is reported as one — an artefact the registry
/// records and this build does not carry is an absence, not a pass.
#[must_use]
pub fn artefact(vendored_path: &str) -> Option<&'static str> {
    ARTEFACTS
        .iter()
        .find(|(path, _)| *path == vendored_path)
        .map(|(_, text)| *text)
}

/// How a diagnostic names an embedded artefact.
///
/// Deliberately not a path. A reader who sees `/schemas/odcs-…json` in a
/// report will go and look at that file, and on an installed binary there is
/// no such file to look at — the bytes are inside the executable. The `embedded:`
/// prefix is what stops the report pointing somewhere that does not exist.
#[must_use]
pub fn artefact_name(vendored_path: &str) -> String {
    format!("embedded:{vendored_path}")
}
