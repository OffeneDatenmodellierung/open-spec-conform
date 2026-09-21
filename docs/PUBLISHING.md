---
Title: Publishing open-spec-conform to crates.io
Space: ARCH
Parent: Runbooks
Layout: article

type: runbook
status: Draft
version: "0.1"
last-modified: 2026-09-21
---

# Publishing to crates.io

## The one fact everything here follows from

**A published version is permanent.** `cargo yank` marks a version as
un-resolvable for new dependants; it does not delete it, and it does not free
the number. Every crate in this family sits at `0.1.0`. If a broken `0.1.0`
goes out, there is no second `0.1.0` — the next release is `0.1.1`, and the
broken one stays on the registry forever with a yank flag on it.

That is why `.github/workflows/release.yml` is `workflow_dispatch` only, why
its dry-run input defaults to *true*, and why a real upload needs the word
`publish` typed into a second box. Merging to `main` publishes nothing.

## What is published, and what is not

Eight of the nine workspace members go to crates.io:

| Crate | Published | Why |
| --- | --- | --- |
| `conform-core` | yes | The vocabulary every other crate speaks. |
| `conform-registry` | yes | Provenance for vendored artefacts. |
| `conform-odcs` | yes | ODCS adapter. |
| `conform-odps` | yes | ODPS adapter. |
| `conform-okf` | yes | OKF adapter. |
| `conform-lexicon` | yes | ODCL adapter. |
| `conform-ffi` | yes | C ABI, and the `wasm` binding. |
| `conform-cli` | yes | The `conform` binary, and the model the site reuses. |
| `conform-web` | **no** | `publish = false` in its manifest. It generates the site; the artefact is an HTML file, and nothing depends on the crate. |

`conform-web` carrying `publish = false` is why the release workflow refuses it
by name rather than silently skipping it.

## The order

The crates form a DAG and the registry enforces it: **a crate cannot be
published until every crate it depends on — including through
`[dev-dependencies]` — is already on crates.io**, because `cargo publish` runs
a verification build of the extracted tarball and that build resolves path
dependencies from the registry, not from this workspace.

```
conform-core
  └── conform-registry
        ├── conform-odcs ─┐
        ├── conform-odps ─┤
        ├── conform-okf   │   (dev-depends on conform-registry)
        └── conform-lexicon
                          ├── conform-ffi   (core, registry, odcs, odps; dev: odcs)
                          └── conform-cli   (core, registry, and all four adapters)
```

The sequence the workflow uses, which is a safe linearisation of that graph:

1. `conform-core`
2. `conform-registry`
3. `conform-odcs`
4. `conform-odps`
5. `conform-okf`
6. `conform-lexicon`
7. `conform-ffi`
8. `conform-cli`

Steps 3–6 are independent of each other and may go in any order among
themselves. Step 7 needs only 1–4; it is placed after 6 so that a single pass
down the list is always correct.

## What can be verified today, and what cannot

As of 2026-09-21, with nothing yet on crates.io:

- `cargo package -p conform-core` — **passes**, including its verification
  build.
- `cargo package` for the other eight — **fails**, every one with
  `no matching package named 'conform-core' found / location searched:
  crates.io index`.

That failure is the ordering rule above showing through. It is **not** a defect
in those crates and it is not something to work around; it disappears one crate
at a time as the sequence proceeds.

So there is no way to dry-run the whole workspace before the first release.
What *can* be checked ahead of time, and is:

- `cargo package --list` works for all nine and shows exactly what each tarball
  would contain.
- Copying precisely those files into a tree that has **no repository root** —
  no `specs.toml`, no `schemas/` — and building every target proves that no
  crate reaches outside its own directory at build time. This is how the
  `conform-ffi` `include_str!` defect was found and how its fix was confirmed.

## The first publish, step by step

The first release is the risky one, because the dry run cannot cover it.

1. Make sure `main` is green: `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
   `cargo test --workspace`, `cargo test --workspace --all-features`,
   `cargo deny --all-features check`.
2. Run **Release** with `crates: conform-core`, `dry_run: true`. It should
   pass. Nothing is uploaded.
3. Run **Release** with `crates: conform-core`, `dry_run: false`,
   `confirm: publish`. `conform-core 0.1.0` is now permanent.
4. Run **Release** with `crates: conform-registry`, `dry_run: true`. This now
   passes, because step 3 put its dependency on the registry. If it does not
   pass, **stop** — the tree is telling you something and the next crate is
   not ready.
5. Repeat the dry-run-then-publish pair for each remaining crate, in the order
   above.

Publishing one crate per dispatch is slower than `crates: all` and is the right
way to do it the first time: every step is preceded by a dry run that could not
have been performed until the step before it completed.

From the second release onward, `crates: all` with `dry_run: true` is a
meaningful whole-workspace check, because every dependency is already on the
registry.

## If a publish fails part-way through

This is the scenario that actually bites, so it gets a written answer.

Say the sequence reaches `conform-odps` and fails. `conform-core`,
`conform-registry` and `conform-odcs` are on crates.io at `0.1.0` and **cannot
be withdrawn**. The workspace is half-published.

**This is not an emergency, and the instinct to "undo" it is the thing to
resist.** What is on the registry is correct — it passed its own verification
build. Nothing downstream is broken, because nothing downstream exists yet.

Do this:

1. **Do not yank the published crates.** Yanking `conform-core 0.1.0` would
   break `conform-registry 0.1.0`, which depends on it and is already public.
   Yank propagates outward as breakage; it does not roll anything back.
2. **Read the actual error.** Two kinds occur here, and they want opposite
   responses:
   - *Transient* — a registry timeout, a rate limit, an index-propagation
     wait that expired. Re-dispatch the workflow for the same crate. Nothing
     needs fixing.
   - *Real* — the verification build failed, or `cargo deny` objected, or a
     manifest is wrong. The crate is not publishable as written.
3. **For a real failure, fix it on a branch and bump only that crate.** The
   failing crate has never been published, so its `0.1.0` is still free — fix
   it and publish `0.1.0` as planned. This is the common case and it costs
   nothing.
4. **If the fault is in an already-published crate**, its version is spent.
   Fix it, bump that crate to `0.1.1`, publish the fix, and then update the
   dependency requirement in the crates that had not gone out yet. Because
   every crate here carries a literal version and depends on its siblings with
   an explicit `version = "0.1.0"`, that edit is mechanical and local. Yank the
   broken version *only* if it is actively harmful — and only after its
   replacement is published, because yanking first leaves a window in which
   the already-public dependants resolve to nothing.
5. **Resume from where it stopped**, not from the beginning. Dispatch the
   workflow with `crates` set to the remaining names; it will reorder them into
   dependency order and refuse anything that is not publishable.

The thing that makes all of this recoverable is that the crates are
independently versioned (FR-011). A workspace on one shared version would have
to burn one number for all eight.

## Repository settings worth turning on

The workflow references two GitHub Environments: `crates-io-dry-run` and
`crates-io`. Referencing an environment that does not exist creates it
**unprotected**, so the names alone guarantee nothing. Two settings on
`crates-io` (Settings → Environments) turn them into a real gate:

- **Required reviewers.** A real publish then waits for a human approval
  distinct from whoever pressed the button.
- **Deployment branches: `main` only.** Without this, a release can be
  dispatched from any branch — including one whose `release.yml` has had the
  confirmation step edited out.

`CARGO_REGISTRY_TOKEN` should be scoped as narrowly as crates.io allows: a
publish-only token, restricted to the `conform-*` crates once they exist. It is
bound to the single uploading step in `release.yml` and appears nowhere else.

## How this relates to `release-plz`

`release-plz.toml` configures changelog generation, GitHub releases, and
`cargo-semver-checks`. Nothing in this repository runs `release-plz` today, and
`release.yml` deliberately does not: the first release needs an explicit,
readable, one-crate-at-a-time sequence rather than a tool's inferred order.

The safe way to automate the *rest* of it is a `release-plz release-pr` job —
it opens a pull request bumping versions and writing changelogs, and publishes
nothing. Pairing that with this workflow gives automation over version
bookkeeping while a human still presses publish. Adding it is a separate change
and is not done here.

One note on `semver_check = true`: `cargo-semver-checks` compares against the
previous version **on crates.io**. Before the first release there is no
baseline, so it has nothing to check and does not block. It starts doing real
work from the second release onward, which is exactly when it is wanted.

## What the tarballs contain

Two tests enforce the parts that are easy to get silently wrong, and both run
in `cargo test --workspace` and again by name in the release gates:

- `crates/conform-web/tests/every_crate_ships_its_licence.rs` — every member
  packages `LICENSE-MIT` and `LICENSE-APACHE`, and the bytes are the
  repository's own. Each crate directory holds symbolic links to the single
  copy at the repository root; cargo dereferences them when packaging, so the
  tarball carries real files while the repository keeps one copy of each
  licence.
- `crates/conform-web/tests/vendored_bytes_are_packaged_not_forked.rs` — no
  crate directory holds a *diverging* copy of `specs.toml` or of a file under
  `schemas/`. `conform-ffi` must carry those bytes inside its own directory for
  its `wasm` feature to build once published, and it does so by symbolic link
  for the same reason: a second copy with its own future is the defect this
  registry exists to prevent.

Both tests fail on a checkout whose filesystem does not support symbolic links,
where each link becomes a short text file holding its own path. That is
deliberate — publishing from such a checkout would ship tarballs whose
`LICENSE-MIT` says `../../LICENSE-MIT` and whose embedded schema is twenty
bytes of path. **Publish from Linux or macOS.**
