# conform-registry

Machine-readable provenance for vendored specification artefacts: where a
schema came from, who stewards it, which immutable upstream revision it was
taken at, and whether the bytes on disk are still those bytes.

A repository that validates documents against a published standard ends up
holding a copy of that standard's schema. The copy is the problem. It is a
snapshot of a moving document, and unless something records *which* moment it
is a snapshot of, nobody can answer the only question that matters about it: is
this still what upstream says?

This crate is that something. `specs.toml` at the repository root is the record;
this crate reads it, checks it against its own rules, and re-hashes every
artefact it describes.

## The two invariants

**Nothing may be pinned to a moving target.** `latest`, `main`, `HEAD` and
their relatives name whatever upstream holds today, so a copy pinned to one of
them cannot be checked against anything, ever. This is not hypothetical: the
schema that motivated this crate is vendored upstream as
`odps-json-schema-latest.json`, with no version, no source URL, no fetch date
and no `$id`. A test fails the build if the word reaches `specs.toml`, and a
negative control proves that test fires.

**A recorded unknown beats a plausible guess.** Every provenance field is
optional, because upstream sometimes genuinely does not publish a licence or a
homepage. Leaving one out is reported as a warning, and an entry with any gap
must carry `notes` saying what was looked at and what it did not say —
an unexplained gap is an error. What the format has no way to express is a
confident-looking value nobody checked.

## The three questions, deliberately separate

```rust
use conform_core::GatePolicy;
use conform_registry::Registry;

// 1. Is this a registry at all? Reads the registry file, nothing else.
let registry = Registry::load_path("specs.toml")?;

// 2. Does it record what a registry must record? Pure: no I/O, no clock.
let rules = registry.validate();

// 3. Do the bytes still match? Re-hashes every artefact on disk.
let integrity = registry.verify();

assert!(!rules.should_gate(GatePolicy::default()));
assert!(!integrity.should_gate(GatePolicy::default()));
```

They fail for unrelated reasons and callers usually want them apart: a CI job
may validate on every build but verify only where the artefacts are checked
out. Malformed input comes back as `conform-core` diagnostics — code, severity,
line, pointer — never as a panic and never as a bare `String`.

## Offline by construction

Nothing here performs network access. An entry may record a `poll.endpoint`
saying *how* to ask upstream whether something newer exists, but asking is a
scheduled job that opens an issue — never a check that fails somebody's
unrelated pull request. A gate that cries wolf is a gate that gets switched
off.

## Dependencies

`conform-core`, a TOML parser, `serde` for the derive, and `sha2`. Byte
integrity is this crate's central claim, and a hand-rolled compression function
is the last thing a reviewer of a provenance crate should have to audit.

## Licence

Dual licensed under `MIT OR Apache-2.0`.
