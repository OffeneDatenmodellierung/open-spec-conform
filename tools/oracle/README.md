# tools/oracle

Records what the **pre-existing** validators in `data-modelling-sdk` say about
the adapter crates' fixture corpora, so `conform-odcs`, `conform-odps` and
`conform-lexicon` can be tested against the thing they replace rather than
against a description of it.

## What it actually does

It calls the real functions:

```rust
data_modelling_core::validation::schema::validate_odcs_internal(content) -> Result<(), String>
data_modelling_core::validation::schema::validate_odps_internal(content) -> Result<(), String>
data_modelling_core::validation::schema::validate_odcl_internal(content) -> Result<(), String>
```

compiled from `../../../data-modelling-sdk/crates/core` with
`--no-default-features --features schema-validation`, over every file in each
crate's `tests/fixtures/`, and writes what they returned to

- `crates/conform-odcs/tests/oracle/odcs-verdicts.json`
- `crates/conform-odps/tests/oracle/odps-verdicts.json`
- `crates/conform-lexicon/tests/oracle/odcl-verdicts.json`

Nothing here reimplements, ports or paraphrases the oracle's rules. The
`detail` field of every record is the `String` that function actually returned.

Regenerate from the repository root:

```sh
cargo run --manifest-path tools/oracle/Cargo.toml
```

## Why it is not a dev-dependency of the crates it serves

This is the honest limitation, and it is worth stating plainly rather than
leaving a reader to infer that the oracle was never run.

A live call would be better. It is not possible under this repository's
existing gates. `cargo deny` resolves the workspace graph **including
dev-dependencies**, and `data-modelling-core` brings in `yaml-rust 0.4.5`:

```text
error[unmaintained]: yaml-rust is unmaintained.
├ ID: RUSTSEC-2024-0320
├ Advisory: https://rustsec.org/advisories/RUSTSEC-2024-0320
├ Solution: No safe upgrade is available!
├ yaml-rust v0.4.5
  └── data-modelling-core v2.4.0
```

That is a true finding about the SDK's supply chain, not noise. Silencing it in
`deny.toml` so a test could compile would trade a real signal for a
convenience. So this crate lives **outside** the workspace — its own
`[workspace]` table, not matched by `crates/*` — and the gated graph never sees
the SDK.

## What stops the recording going stale

Three things, each enforced by a test in the adapter crates'
`tests/oracle_agreement.rs`:

1. **Coverage, both ways.** Every fixture on disk must have a record and every
   record a fixture. A new fixture with no recorded verdict fails the build
   rather than being silently skipped.
2. **A SHA-256 per fixture.** Each record carries the digest of the bytes the
   oracle was given. The test re-hashes the file; edit a fixture without
   re-running this tool and the test fails rather than comparing against a
   verdict for a document that no longer exists.
3. **The schema digests on both sides.** Each recording carries the digest of
   the schema the oracle `include_str!`s and the digest of the schema this
   repository vendors, and the test asserts they are equal. So a disagreement
   is always about *rules* and never about two validators having been handed
   different bytes. (For ODPS this is load-bearing: the SDK's copy is still
   named `odps-json-schema-latest.json` while ours is
   `odps-json-schema-v1.0.0.json`. The digests prove they are the same file.)

Each recording also names the SDK commit and crate version it was taken from,
so "which implementation was this recorded against" has an answer.

## Assumptions

`data-modelling-sdk` is a **sibling checkout** of this repository. That is an
assumption about one machine's layout, and it is why this tool is not part of
the workspace build: nothing in `cargo test`, `cargo clippy` or `cargo deny`
depends on the SDK being present. Only regenerating the recording does.
