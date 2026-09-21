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
   `table`, `column`, `domain`, and the rest) does **not** move here —
   **agreed by the human, 2026-09-21.** See `0002-sdk-models-coupling.md` §4,
   where the objection was raised on naming grounds. The dependency direction
   settles it on stronger grounds: those are the SDK's own domain concepts, the
   SDK is the dependent party, and a dependent keeps its own domain model. This
   repository supplies models of *published specifications* — ODCS, ODPS, CADS,
   DBMV — and nothing else. The proposed `dmsdk-model-*` crates are therefore
   not needed and will not be created. **The model port is complete.**
4. The `yaml-rust` removal in the SDK (merged 2026-09-21, `66c7335`) remains
   worth having on its own merits, but it is no longer a precondition for
   anything here.

## Roteiro inherits too

Confirmed by the human, 2026-09-21: **Roteiro will also depend on this
repository**, via the published crates. The same rule applies — nothing here
may ever depend on Roteiro.

One dependency could have created a cycle and does not. `conform-okf` depends
on `okf-core 0.2.7`, and Roteiro's `rto-render` and `rto-okf-syntax` depend on
`okf-core 0.2.6`. If `okf-core` were Roteiro's crate, `Roteiro → conform-okf →
okf-core` would close a loop. It is not: crates.io gives its repository as
`https://github.com/W4G1/okf`, an independent author, and `conform-okf`'s own
manifest already says as much. The edge is to a shared third party, which is
the one shape that is always safe.

`conform-okf` was migrated out of Roteiro's `rto-render/src/okf/`, so on
adoption Roteiro deletes that module rather than keeping a second copy.

### Decided — `rto-okf-syntax` moves here as `conform-okf-syntax`

Roteiro published `rto-okf-syntax 0.1.1`, whose own header said it was "written
to be given away" and was "meant to be deleted if upstream adopts it". It
checks the fenced code blocks in an OKF bundle, with `tree-sitter` and
`sqlparser` behind optional features so no consumer pays for parsers it does
not use.

That is precisely the gap `conform-okf` documents in itself: the two
code-parsing checks were "absent by design, and their absence is the only
intended behavioural difference from upstream over the published corpus".

Under this decision, **we are the upstream**, and the human decided on
2026-09-21 that it moves. It is here as `crates/conform-okf-syntax`, ported
from Roteiro at `8817904e`, and `conform-okf` reaches it through a default-off
`syntax` feature. Roteiro deleting its copy is a separate change.

Two things were raised against it when it was still open, and this is what
happened to each.

**It is already published, so this is a rename on crates.io.** It is, and the
new crate debuts at `0.1.0` rather than continuing `0.1.1`: a new name is a new
registry entry whose version history starts where it starts, and continuing
somebody else's numbering across a rename claims a release history this name
has not had. `rto-okf-syntax 0.1.1` stays where it is, with its 256 downloads,
until Roteiro deprecates it. Nothing is yanked and nothing breaks; a dependant
of the old name keeps a working crate and gains a pointer.

**It would make `conform-okf` optionally heavier, and that is a line to cross
deliberately.** It was crossed deliberately and the cost was measured rather
than assumed:

- `cargo tree -p conform-okf` at default features is **byte-identical** to what
  it was before, dev- and build-dependencies included. `cargo tree -p
  conform-core` is still a single node.
- `--features syntax` is the cheap tier and adds **no parser at all** — the
  dependency is taken with `default-features = false`, so it costs one
  pure-Rust crate whose own dependencies are `okf-core`, which `conform-okf`
  already had, and `serde_json`, which the lockfile already had. It is enough
  to check JSON, YAML and shell quoting.
- `--features syntax-sql` and `--features syntax-grammars` are what add
  `sqlparser` and the tree-sitter grammars. Eighteen third-party crates between
  them, every licence already on `deny.toml`'s allow-list, and
  `cargo deny --all-features check` passes with the same five warnings it had
  before — no allow-list edit, no exception, no advisory.

The rules are a third `Validator` rather than a `cfg` inside `OkfHygiene`, so
the two existing checks report identically in every feature configuration. A
feature that edited an existing check would make its output a function of the
build, and because cargo unifies features across a graph the consumer who got
the different answer would not be the one who asked for it.
