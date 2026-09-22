---
Title: Publishing open-spec-conform to crates.io
Space: ARCH
Parent: Runbooks
Layout: article

type: runbook
status: Draft
version: "0.3"
last-modified: 2026-09-22
---

# Publishing to crates.io

## The one fact everything here follows from

**A published version is permanent.** `cargo yank` marks a version
un-resolvable for new dependants; it does not delete it, and it does not free
the number. If a broken version goes out, that number is spent — the next
release is a bump, and the broken one stays on the registry forever with a yank
flag on it.

That is why `.github/workflows/release.yml` is `workflow_dispatch` only, why
its dry-run input defaults to *true*, and why a real upload needs the word
`publish` typed into a second box. Merging to `main` publishes nothing.

A second fact, learned the hard way from a dependency and worth stating before
anything else: **a fix that is merged but not released does not exist for your
consumers.** See [What the registry says, not what the source says]
(#what-the-registry-says-not-what-the-source-says) below.

## `conform-cli` is the point

Twelve of the thirteen crates here are components. `conform-cli` is the
artefact this project exists to deliver: the `conform` binary a person installs
to check a data contract. It is also the crate that publishes **last**, because
it depends on nearly everything else.

Those two facts together are the risk in this runbook. If a release sequence
fails part way, **the flagship is the crate that does not make it** — the
components go out and the thing anybody actually wanted does not. Plan the
sequence so that the last step is the one you are most confident about, and do
not start a release you do not intend to finish.

`cargo install conform-cli` installs a binary named `conform`. The crate name
is `conform-cli`, and only the `[[bin]]` target is called `conform`. The bare
name `conform` on crates.io is taken by an abandoned 2018 macro crate and is
not worth pursuing.

> **Option, not a recommendation.** `open-spec-conform` is free on crates.io.
> If matching the repository name is worth more for discoverability than
> `conform-cli`'s directness, that name is available — but it is a decision
> about product naming, not about packaging, and nothing here depends on it.

### What an installed binary can and cannot do

An earlier version of this runbook said the `cargo install conform-cli`
journey "works and needs no change". **That was wrong**, and it was wrong on
the point that matters: the installed binary could not check anything. It
searched for a `specs.toml` at or above the working directory, found none, and
exited 2 under `CLI100`. The message was honest and the tool was useless.

`conform-cli` now carries the catalogue and every artefact it records, as
symbolic links under `crates/conform-cli/embedded/` that cargo dereferences
into the tarball. So:

**It can**, with nothing on disk:

- validate ODCS, ODPS and ODCL documents, and check OKF bundles;
- re-hash every embedded artefact against the embedded catalogue's digests —
  the *same* comparison the on-disk path uses, through
  `conform_registry::verify_bytes`, not a second copy of it;
- run `registry list` and `registry verify` over the embedded catalogue;
- say on every run which registry answered.

**It cannot**:

- speak for any `specs.toml` on disk. Both sides of the embedded comparison
  were frozen into the executable at the same moment, so a green embedded
  `registry verify` proves the *binary* is internally consistent and nothing
  about the working tree. The provenance sentence on every embedded report
  says exactly that.
- see anything vendored after it was published — see below.

### The embedded catalogue ages, and that is the deal

**An installed `conform 0.1.0` carries `0.1.0`'s view of upstream forever.**
If ODCS 3.2 ships next year, a `conform 0.1.0` installed today still validates
against 3.1.0, and still reports `odcs 3.1.0` while doing it.

This is the right default — a frozen catalogue is what makes an embedded
verdict reproducible — but it is a property users have to be told, not a
footnote. Both escapes are ordinary and the tool names them:

- `cargo install conform-cli --force` takes a newer release with a newer
  catalogue.
- `--registry <path>` checks against a catalogue the user controls, and beats
  the embedded one. The `registry:` line then names the file.

The precedence is explicit and stated in the output on every run: `--registry`
first, then a `specs.toml` discovered by searching upward, then the embedded
copy. That ordering is not a convenience. A binary that answered from its own
memory while the reader stood in a checkout would report the specifications it
was built with, and the reader would believe they were seeing the ones in front
of them — which is the precise failure this project exists to prevent. So the
embedded copy is reached last, and the run says when it was reached.

## What is published, and what is not

Thirteen workspace members; twelve go to crates.io.

| Crate | Published | Why |
| --- | --- | --- |
| `conform-core` | yes | The vocabulary every other crate speaks. Zero non-optional dependencies. |
| `conform-registry` | yes | Provenance for vendored artefacts. |
| `conform-odcs` | yes | ODCS conformance adapter. |
| `conform-odps` | yes | ODPS conformance adapter. |
| `conform-okf` | yes | OKF conformance adapter. |
| `conform-okf-syntax` | yes | Code syntax for OKF fenced blocks. Optional from `conform-okf`, and useful on its own to anyone who cannot take `okf-validator`'s tree — it was written to be given away and this is the second time it has been. |
| `conform-lexicon` | yes | ODCL conformance adapter. |
| `conform-model-odcs` | yes | Typed ODCS model. No internal dependencies. |
| `conform-model-odps` | yes | Typed ODPS model. No internal dependencies. |
| `conform-model-cads` | yes | Typed CADS model. No internal dependencies. |
| `conform-model-dbmv` | yes | Typed DBMV model. No internal dependencies. |
| `conform-ffi` | yes | C ABI, and the `wasm` binding. |
| **`conform-cli`** | **yes** | **The flagship.** The `conform` binary. |
| `conform-web` | **no** | `publish = false`. It generates the catalogue site; the artefact is an HTML file and nothing depends on the crate. |

## The order is computed, not written down

A crate cannot be published until every crate it depends on — **including
through `[dev-dependencies]`** — is already on crates.io, because
`cargo publish` runs a verification build of the extracted tarball and that
build resolves path dependencies from the registry, not from this workspace.

`tools/publish-order.py` derives a safe sequence from `cargo metadata` and
drops anything carrying `publish = false`. `release.yml` calls it. **Do not
transcribe its output into a workflow or a script** — this workspace went from
nine crates to thirteen between two commits, and to fourteen in the one after
that, and a list written down would have gone on publishing the first nine
while looking complete. `conform-okf-syntax` arriving is the most recent
demonstration: it has to precede `conform-okf`, and nobody had to be told.

```console
$ ./tools/publish-order.py
conform-core conform-model-cads conform-model-dbmv conform-model-odcs
conform-model-odps conform-okf-syntax conform-registry conform-lexicon
conform-odcs conform-odps conform-okf conform-cli conform-ffi
```

That is what it prints today, shown so a reader knows the shape — not an
assertion that it will not change. Crates within a tier are independent of each
other; the four `conform-model-*` crates have no internal dependencies at all
and could go first, last, or any time.

## What can be verified today, and what cannot

> **This section describes the state before the first release, and that state
> is past.** Since 2026-09-21, `conform-core` and the four `conform-model-*`
> crates are on crates.io at `0.1.0`; the eight below them are not. What
> follows is still the right explanation of *why* a crate cannot be dry-run
> before its dependencies exist, and the wall it describes now falls away one
> crate at a time. For what is publishable this minute, ask rather than read:
> `./tools/publish-status.py $(./tools/publish-order.py)`.

With nothing yet on crates.io:

- **Six crates package cleanly today**, verification build included:
  `conform-core`, the four `conform-model-*` crates and `conform-okf-syntax`.
  All six depend on no other workspace member, which is exactly why —
  `conform-okf-syntax` takes `okf-core` and `serde_json` from crates.io and
  nothing from here, which is also what makes it giveable away.
- The other eight **fail**, every one with `no matching package named '…'
  found / location searched: crates.io index` — `conform-core` for seven of
  them, `conform-cli` for `conform-web`.

That is the ordering rule showing through. It is **not** a defect in those
crates, and it disappears one crate at a time as the sequence proceeds. It does
mean **there is no way to dry-run the whole workspace before the first
release**, and no tool changes that — `release-plz release --dry-run` hits the
same wall.

What *is* checkable ahead of time, and was:

- `cargo package --list` works for all fourteen and reports exactly what each
  tarball would hold.
- Copying precisely those files into a tree with **no repository root** — no
  `specs.toml`, no `schemas/` — and building every target proves no crate
  reaches outside its own directory at build time. All fourteen build; so does
  `conform-ffi --features wasm`. This is how the `conform-ffi` `include_str!`
  defect was found and how its fix was confirmed, and it is worth re-running
  whenever a crate starts embedding something.

### A published crate's test suite is not expected to run standalone

Running `cargo test` inside an unpacked tarball gives, today:

| Crate | From its own tarball |
| --- | --- |
| `conform-core` | 18 pass, 0 fail |
| `conform-model-odcs` / `-odps` / `-cads` / `-dbmv` | all pass (28 / 10 / 12 / 14) |
| `conform-registry` | 15 pass, 11 fail |
| `conform-odcs` / `conform-odps` | 8 pass, 14 fail each |
| `conform-okf-syntax` | 24 pass, 0 fail |
| `conform-okf` | 27 pass, 3 fail |
| `conform-lexicon` | 20 pass, 27 fail |
| `conform-ffi` | 27 pass, 15 fail |
| `conform-cli` | 21 pass, 28 fail |

Every failure is the same one: `tests/support/mod.rs` climbs two directories to
find the workspace root's `specs.toml`, and a tarball has no workspace root.

`conform-okf-syntax` is the exception and is worth a sentence, because it was
*not* an exception when it arrived. Its manifest guard —
`dependencies_are_frozen`, the test that is the whole reason the crate is worth
having — read `[dependencies]` as a table of one-line entries, which is how this
repository writes it and **not** how `cargo package` ships it: the rewritten
manifest spells each entry `[dependencies.NAME]`, so inside the tarball the
guard parsed nothing. It failed loudly rather than silently, because the list it
compares against is not empty, but a consumer who unpacked the crate and ran its
tests would have seen the supply-chain guard fail for a reason that had nothing
to do with the supply chain. The reader now understands both spellings and
`the_manifest_reader_understands_a_packaged_manifest` pins the one this
workspace never produces. The crate it was ported from,
`rto-okf-syntax 0.1.1`, still has the original.

**This does not block publishing.** `cargo publish` *builds* test targets during
verification and never *runs* them, and the builds all succeed. What it means is
that a consumer who unpacks a crate and runs its tests will see failures that
say nothing about the crate. Nothing here is being hidden; it is recorded so the
first bug report about it has an answer. Fixing it properly would mean either
shipping a registry inside each tarball — a second copy of the vendored bytes,
which is the defect this project exists to prevent — or teaching the helpers to
skip when no registry is reachable, which is a change to seven crates' test code
and not packaging work.

## The first publish, step by step

The first release is the risky one, because the dry run cannot cover it.

1. Make sure `main` is green: `cargo fmt --all --check`;
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
   `cargo test --workspace`; `cargo test --workspace --all-features`;
   `cargo deny --all-features check`. The release workflow runs all of these as
   a prerequisite anyway, and will not reach the publish step without them.
2. Run **Release** with `crates: conform-core`, `dry_run: true`. It should
   pass. Nothing is uploaded.
3. Run **Release** with `crates: conform-core`, `dry_run: false`,
   `confirm: publish`. `conform-core 0.1.0` is now permanent.
4. Run **Release** with `crates: conform-registry`, `dry_run: true`. This now
   passes, because step 3 put its dependency on the registry. If it does not
   pass, **stop** — the tree is telling you something.
5. Repeat the dry-run-then-publish pair for each remaining crate, in the order
   `tools/publish-order.py` prints.

The four `conform-model-*` crates depend on nothing internal, so they can be
done at any point and are a low-risk way to exercise the workflow for real
before the interdependent crates start. They already pass a full `cargo
package` today, which no other crate but `conform-core` does.

Publishing one crate per dispatch is slower than `crates: all` and is the right
way to do it the first time: every step is preceded by a dry run that could not
have been performed until the step before completed.

From the second release onward, `crates: all` with `dry_run: true` is a
meaningful whole-workspace check.

## If a publish fails part-way through

This is the scenario that actually bites, so it gets a written answer — and on
**2026-09-21** it stopped being hypothetical. See
[The half-published release of 2026-09-21](#the-half-published-release-of-2026-09-21)
below for what happened and what changed because of it.

Say the sequence reaches `conform-odps` and fails. `conform-core`,
`conform-registry` and the model crates are on crates.io at `0.1.0` and
**cannot be withdrawn**. The workspace is half-published, and `conform-cli` —
the one anybody wanted — is not out.

**This is not an emergency, and the instinct to "undo" it is the thing to
resist.** What is on the registry is correct: it passed its own verification
build. Nothing downstream is broken, because nothing downstream exists yet.

1. **Do not yank the published crates.** Yanking `conform-core 0.1.0` would
   break `conform-registry 0.1.0`, which depends on it and is already public.
   Yank propagates outward as breakage; it does not roll anything back.
2. **Read the actual error.** Three kinds occur, wanting different responses:
   - *Rate-limited* — crates.io refused because too many **new** crates were
     created too quickly. The workflow now handles this itself; you only see
     it if its wait budget ran out. Nothing needs fixing. See
     [The new-crate rate limit](#the-new-crate-rate-limit).
   - *Transient* — a registry timeout, or an index-propagation wait.
     Re-dispatch; nothing needs fixing.
   - *Real* — the verification build failed, `cargo deny` objected, or a
     manifest is wrong. The crate is not publishable as written.
3. **For a real failure, fix it and publish the same version.** The failing
   crate has never been published, so its `0.1.0` is still free. This is the
   common case and it costs nothing.
4. **If the fault is in an already-published crate**, its version is spent. Fix
   it, bump *that crate* to `0.1.1`, publish the fix, then update the
   dependency requirement in the crates that had not gone out yet. Because every
   crate carries a literal version and depends on its siblings with an explicit
   `version = "0.1.0"`, that edit is mechanical and local. Yank the broken
   version **only** if it is actively harmful, and only *after* its replacement
   is published — yanking first leaves a window in which already-public
   dependants resolve to nothing.
5. **Resume by dispatching again with `crates: all`.** Both loops ask the
   crates.io index whether each exact `name@version` is already there, and skip
   it if so. Re-dispatching does not republish anything and cannot fail because
   a previous run succeeded; it picks up whatever is outstanding. Naming the
   remaining crates individually still works and is equivalent — `all` is
   simply the answer that cannot be got wrong.

What makes this recoverable is that the crates are independently versioned
(FR-011). A workspace on one shared version would have to burn one number for
all twelve.

### The new-crate rate limit

crates.io limits how fast **new crate names** can be created — not how fast
versions of an existing crate can be published. From crates.io's own
`LimitedAction::PublishNew`: a burst of **5**, refilling at **one every 10
minutes**. `PublishUpdate`, which is what every release after the first one
does, is a burst of 30 refilling once a minute and will never be felt here.

So the *first* release of this workspace is the one release that is guaranteed
to hit it: thirteen new names, and only five may be created before the
throttle starts. A full first release from an empty bucket needs roughly **80
minutes of waiting**, spread across the last eight crates.

The refusal is an HTTP 429 whose body names the moment the next crate is due:

```
You have published too many new crates in a short period of time. Please try
again after Mon, 22 Sep 2026 13:24:45 GMT and see
https://crates.io/docs/rate-limits for more details.
```

`cargo` prints that sentence, and `release.yml` reads the time out of it and
waits until then rather than guessing at an interval — the bucket is often
part-refilled already, so guessing over-waits. The `max_wait_minutes` dispatch
input bounds how long the whole run may spend doing this (default **90**,
enough for a full first release). When the budget runs out the run **fails**
and names what is left, rather than waiting past its own job timeout and being
killed with nothing printed.

Nothing about a rate limit means anything is wrong with the tree. The only
decision it asks for is whether to wait or to come back later, and coming back
later is free now that re-dispatching resumes.

### The half-published release of 2026-09-21

The first real release run published five crates — `conform-core` and the four
`conform-model-*` crates — and was then refused on the sixth by the new-crate
limit, exactly as the burst of five predicts.

Every rerun after that failed **instantly**, on the first crate, with:

```
error: crate conform-core@0.1.0 already exists on crates.io index
Process completed with exit code 101
```

Both loops walked the full computed sequence and ran `cargo publish`
unconditionally, so the first already-published crate ended the run before
reaching any of the eight that still needed creating. Five crates were
permanent, eight were uncreatable, and there was no dispatch that made
progress — naming only the missing crates would have worked, but the
documented recovery said `crates: all`, and `all` could no longer run at all.

Three things changed, and they are worth stating as properties rather than as
a changelog:

- **The index is the authority on what is published.** Before each crate,
  `tools/publish-status.py` asks the sparse index whether that exact
  `name@version` is present, and the crate is skipped if it is. It is *not*
  decided by matching cargo's error text: `already exists` is a plausible
  substring of failures that mean something else, and a loop that reads "the
  word appeared" as "carry on" reports releases finished that are not. Only an
  exact version match skips — a crate whose name is taken but whose version is
  new still publishes. A yanked version counts as published, because the number
  is spent either way.
- **A rate limit is waited out, not failed on.** Described above.
- **A green tick means finished.** The publish step ends by asking the index
  what is actually there and printing three lists — PUBLISHED THIS RUN,
  SKIPPED (already on index), STILL MISSING — and **fails if anything asked
  for is missing**. The previous workflow would exit 0 having published
  nothing at all.

The dry run skips already-published crates for the same reason, and that makes
it *more* useful rather than less: this tree cannot change a tarball that has
already shipped, so re-verifying one proves nothing. What a dry run is being
asked in a half-published workspace is whether what *remains* is fit to
publish, and that is now exactly what it checks.

You can ask the same question by hand at any time:

```console
$ ./tools/publish-status.py $(./tools/publish-order.py)
skip     conform-core        0.1.0  conform-core@0.1.0 is already on the crates.io index
...
publish  conform-registry    0.1.0  conform-registry is not on the crates.io index
```

## What the registry says, not what the source says

`data-modelling-core` is published at `2.4.0`, and **that published version
still declares `yaml-rust ^0.4`** — confirmed against
`crates.io/api/v1/crates/data-modelling-core/2.4.0/dependencies`, which also
still lists `serde_yaml ^0.9`, itself unmaintained. The removal was merged into
the SDK but the version was not bumped, so the source at `2.4.0` now differs
from what the registry serves under `2.4.0`.

Two consequences:

- Anything depending on the *registry* copy still inherits RUSTSEC-2024-0320.
  The advisory is cleared for consumers only when the SDK publishes a new
  version. This is why `tools/oracle` cannot simply move into this workspace
  and depend on the released crate — it is a precondition, not a preference.
- A future `cargo publish` of `2.4.0` from that repository would be rejected
  outright, because the version already exists.

The general lesson applies to us from the moment we publish: **merged is not
released.** A fix on `main` does nothing for anybody consuming `0.1.0`. Bump
and publish, or the fix does not exist.

## Repository settings worth turning on

`release.yml` references two GitHub Environments, `crates-io-dry-run` and
`crates-io`. Referencing an environment that does not exist creates it
**unprotected**, so the names alone guarantee nothing. Two settings on
`crates-io` (Settings → Environments) make them a real gate:

- **Required reviewers** — a real publish then waits for a human approval
  distinct from whoever pressed the button.
- **Deployment branches: `main` only** — without this, a release can be
  dispatched from any branch, including one whose `release.yml` has had the
  confirmation step edited out.

`CARGO_REGISTRY_TOKEN` should be as narrow as crates.io allows: publish-only,
and scoped to the `conform-*` crates once they exist. It is bound to the single
uploading step in `release.yml` and appears nowhere else.

`ci.yml` is safe with respect to that secret: it triggers on `pull_request`,
not `pull_request_target`, so a fork's pull request runs unprivileged and
receives no secrets. Nothing in it references `secrets.*`.

## How this relates to `release-plz`

`release-plz.toml` configures changelog generation, GitHub releases, and
`cargo-semver-checks`. Nothing runs `release-plz` today, and `release.yml`
deliberately does not: the first release needs an explicit, one-crate-at-a-time
sequence with a human looking at each step.

The safe way to automate the rest is a `release-plz release-pr` job — it opens
a pull request bumping versions and writing changelogs, and **publishes
nothing**. That pairs well with this workflow: automation over version
bookkeeping, a human over the publish button. Adding it is a separate change.

On `semver_check = true`: `cargo-semver-checks` compares against the previous
version **on crates.io**. Before the first release there is no baseline, so it
has nothing to check and does not block. It starts doing real work from the
second release onward, which is when it is wanted. The configuration is correct
for independently versioned crates and nothing in it misorders a release.

One open question for a human, deliberately not decided here: `publish = false`
stops `conform-web` reaching crates.io, but `release-plz` will still version it,
changelog it and tag it. If that is unwanted, a `[[package]]` block for
`conform-web` with `release = false` turns it off. It is reasonable either way —
the crate *is* a versioned artefact.

## Manifest metadata

Every publishable crate carries `description`, `readme`, `keywords` (five or
fewer, all valid) and `categories`. All seven distinct category slugs used
across the workspace were checked live against `crates.io/api/v1/categories`
and all seven exist; an unknown slug is rejected at upload, so this is worth
re-checking if one is ever added.

`documentation` and `homepage` are unset everywhere, deliberately:

- `documentation` — crates.io links docs.rs automatically. Setting it would add
  a string that says what the default already says and can go stale.
- `homepage` — there is no deployed site URL yet. `repository` already points
  at GitHub; duplicating it into `homepage` adds noise, not information. When
  the generated site has a public URL, that is the correct value and setting it
  then is worthwhile.

## What the tarballs contain

Two tests enforce the parts that are easy to get silently wrong. Both run in
`cargo test --workspace` and again by name in the release gates, and both
enumerate workspace members **dynamically** — when four crates arrived at once,
both failed naming all four, without an edit.

- `crates/conform-web/tests/every_crate_ships_its_licence.rs` — every member
  packages `LICENSE-MIT` and `LICENSE-APACHE`, and the bytes are the
  repository's own. Each crate directory holds symbolic links to the single
  copy at the repository root; cargo dereferences them when packaging, so the
  tarball carries real files while the repository keeps one copy of each.
- `crates/conform-web/tests/vendored_bytes_are_packaged_not_forked.rs` — no
  crate directory holds a *diverging* copy of `specs.toml` or of a file under
  `schemas/`. `conform-ffi` must carry those bytes inside its own directory for
  its `wasm` feature to build once published, and does so by symbolic link for
  the same reason: a second copy with its own future is the defect this registry
  exists to prevent.

Both fail on a checkout whose filesystem does not support symbolic links, where
each link becomes a short text file holding its own path. That is deliberate —
publishing from such a checkout would ship tarballs whose `LICENSE-MIT` reads
`../../LICENSE-MIT` and whose embedded schema is twenty bytes of path.
**Publish from Linux or macOS.**
