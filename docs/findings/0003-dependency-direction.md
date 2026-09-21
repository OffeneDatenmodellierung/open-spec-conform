# 0003 — Dependency direction: the SDK inherits from here

Status: decided
Date: 2026-09-21
Decided by: the human, in conversation

## The decision

**`data-modelling-sdk` will depend on `open-spec-conform`, not the other way
round.** The SDK will be refactored to consume the `conform-*` crates once this
repository is published; this repository must never acquire a runtime or
dev-dependency on the SDK.

## Why this corrects an earlier conclusion

Three findings in this repository, and several turns of planning, treated the
exile of `tools/oracle` from the cargo workspace as a workaround to be undone.
The reasoning was: `tools/oracle` depends on `data-modelling-core` by path;
that drags `yaml-rust 0.4.5` into the graph; `cargo deny` refuses it; therefore
the harness must live outside the workspace until the advisory clears. Clearing
RUSTSEC-2024-0320 was then described as unblocking the "real prize" of bringing
the oracle in.

With the dependency direction settled, that prize is a trap. If the SDK depends
on `conform-*`, and this workspace holds a dev-dependency on
`data-modelling-core`, the estate acquires a dependency cycle at the repository
level. Cargo would reject it outright once both sides are published, and it
would be unresolvable without unpicking whichever side moved last.

**The current arrangement is therefore correct by architecture, not by
accident.** It should be preserved deliberately rather than removed when the
advisory clears.

## What makes this safe today

The differential tests do not need the SDK at test time. Each adapter's
`tests/oracle_agreement.rs` reads a **committed recording**:

- `crates/conform-odcs/tests/oracle/odcs-verdicts.json`
- `crates/conform-odps/tests/oracle/…`
- `crates/conform-lexicon/tests/oracle/…`

`tools/oracle` is the binary that *produced* those recordings, by calling
`validate_odcs_internal` and its siblings directly, with
`--features schema-validation`, over every fixture. It lives outside the
workspace and is run by hand. The recordings are verbatim, per-fixture, and
digest-pinned.

So `cargo test --workspace` is already self-contained. Nothing in the published
crates reaches for the SDK.

## The consequence that needs stating

Once the SDK adopts the `conform-*` crates, **re-running `tools/oracle` compares
this project against itself.** The recordings would stop being evidence and
become a tautology, while still looking like evidence — which is worse than
having none.

The recordings are therefore a **historical artefact**, frozen at the
pre-adoption commits already named in each file. They should not be regenerated
after the SDK migrates. `tools/oracle` should be retained for provenance and
marked as such, not maintained as a live tool.

## Consequences for sequencing

1. This repository publishes to crates.io first. The SDK then depends on
   published versions, not on paths.
2. The SDK's migration happens in one pass, when this repository is ready —
   not incrementally against a moving target.
3. Tier 3 of the model port (`workspace`, `decision`, `knowledge`, `sketch`,
   `table`, `column`, `domain`, and the rest) should **not** move here. See
   `0002-sdk-models-coupling.md` §4, where the objection was raised on naming
   grounds. The dependency direction settles it on stronger grounds: those are
   the SDK's own domain concepts, the SDK is the dependent party, and a
   dependent keeps its own domain model. This repository supplies models of
   *published specifications* — ODCS, ODPS, CADS, DBMV — and nothing else.
4. The `yaml-rust` removal in the SDK (merged 2026-09-21, `66c7335`) remains
   worth having on its own merits, but it is no longer a precondition for
   anything here.
