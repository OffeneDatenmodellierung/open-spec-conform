---
title: Open Spec Conform — implementation plan
status: Draft
version: "0.1"
last-modified: 2026-09-20
owners: [data-platform]
related-adr: docs/adrs/0001-open-spec-conform.md
related-spec: docs/spec/conform-core.spec.md
---

# Open Spec Conform — Implementation Plan

This plan turns ADR-0001 and `docs/spec/conform-core.spec.md` into a buildable
sequence of independently publishable crates, and adds the four surfaces
requested beyond the original spec: a **single-page app**, a **TUI console**, a
**`--json` per-spec diagnostic output**, an **optional FFI layer**, and a
**registry of upstream canonical spec sources**.

Everything in §1 is verified evidence from the existing repositories. Where the
ADR's stated premise does not match what is on disk, §1.4 says so — those
corrections change the sequencing in §7 and are the most important thing in this
document to review first.

---

## 1. Evidence base — what actually exists today

Gathered by read-only inspection of `/Users/mark/GIT/OffeneDatenmodellierung/`.

### 1.1 Roteiro — the OKF validator (the mature reference)

| Fact | Evidence |
|---|---|
| Workspace layout | `crates/` with ten members: `roteiro`, `rto-exec`, `rto-faithful`, `rto-graph`, `rto-llama`, `rto-okf-syntax`, `rto-remote`, `rto-render`, `rto-serve`, `rto-spec` |
| Edition / MSRV | `edition = "2024"`, `rust-version = "1.96"`, `resolver = "3"` |
| Licence | `MIT OR Apache-2.0` (dual, both files at repo root) |
| Repository / homepage | `github.com/OffeneDatenmodellierung/Roteiro`, `roteiro.dev` |
| OKF conformance code | `crates/rto-render/src/okf/conform.rs` |
| Current diagnostic types | `Finding` (`conform.rs:62`), `CheckReport` (`conform.rs:81`) |
| Current entry points | `validate_report`, `validate_bundle`, `lint_report`, `lint_bundle` |
| Differential fixtures | `crates/rto-render/tests/fixtures/okf-upstream/` + `tests/okf_interop.rs` |
| House config present | `deny.toml`, `clippy.toml`, `rustfmt.toml`, `release-plz.toml` |
| TUI / FFI | **None.** No `ratatui`, `crossterm`, `uniffi`, `cbindgen`, `pyo3` or `wasm-bindgen` in any manifest |

Two findings here are load-bearing for this plan.

**(a) The independent-versioning precedent already exists and is documented.**
Nine crates take `version.workspace = true` at `6.0.0`; `rto-okf-syntax` carries
its own `0.1.1`. `release-plz.toml` explains why in a `[[package]]` block:

> `rto-okf-syntax` carries its own version, and is the first crate here to do so.
> The other nine take `version.workspace = true` because they are one product cut
> into pieces… This one is not that. It is written to be given away, it is usable
> by anyone else who cannot take `okf-validator`'s dependency tree, and it is
> meant to be *deleted* if upstream adopts it.

That is precisely the requirement "each new crate should be able to be published
standalone with its own version number." We adopt this pattern wholesale rather
than reinventing it — see §6.2.

**(b) `PROVENANCE.md` is the working prototype of upstream-source tracking.**
`crates/rto-render/tests/fixtures/okf-upstream/PROVENANCE.md` already records
repository URL, upstream path, pinned commit SHA + date, licence, copyright
holder, and a reasoned justification for vendoring over fetching:

| Source | |
|---|---|
| Repository | `https://github.com/GoogleCloudPlatform/open-knowledge-format` |
| Path upstream | `bundles/acme_retail`, `bundles/ga4` |
| Commit | `ad30107c31c06aec8a7d5636e0d1058118604e6f` (2026-08-21) |
| Licence | Apache-2.0 |
| Copyright | Google LLC |

The registry in §3 is a machine-readable generalisation of this file.

### 1.2 data-modelling-sdk — where ODCS/ODPS actually live

Rust, with a `crates/` directory and a top-level `schemas/` directory holding
**vendored JSON Schemas**:

- `schemas/odcs-json-schema-v3.1.0.json` — version-pinned in the filename
- `schemas/odps-json-schema-latest.json` — **not pinned; "latest"**
- `schemas/odcl-json-schema-1.2.1.json`
- `schemas/cads.schema.json`
- plus first-party schemas (`domain-`, `knowledge-`, `decision-`, `sketch-`,
  `workspace-`, `system-`, `common-types-`, `dbmv`)

A duplicate copy of four of these sits under
`specs/003-odcs-field-preservation/schemas/`.

`odps-json-schema-latest.json` is the concrete motivating defect for this whole
workstream: a vendored copy of a moving upstream document, with no recorded
version, no source URL, no fetch date, and no way to detect that upstream has
changed. Nobody can answer "is this current?" without manually diffing against
Bitol. **No `PROVENANCE.md` exists anywhere in this repo.**

### 1.3 Canonical upstream sources found referenced

| Spec | Canonical URL referenced in-repo |
|---|---|
| ODCS | `https://bitol-io.github.io/open-data-contract-standard/latest/` |
| ODPS | `https://bitol-io.github.io/open-data-product-standard` |
| Steward | Bitol — `https://bitol.io/` |
| OKF | `https://github.com/GoogleCloudPlatform/open-knowledge-format` |

These appear as hard-coded hyperlinks scattered across `open-data-modelling`'s
Hugo content and spec documents (≈20 occurrences). They are documentation
strings, not data — which is why the SPA in §5 cannot currently be generated
from anything and has to be hand-maintained today.

### 1.4 Corrections to the ADR's stated premise — **read this before approving**

The ADR opens: *"Three conformance validators exist or are in flight under common
ownership: ODM's ODCS validator, ODM's ODPS validator, and Roteiro's OKF
validator."* The evidence only partly supports that.

1. **`open-data-modelling` is a Hugo documentation site, not a validator.** It
   contains `hugo-site/`, `specs/`, `package.json` and Markdown content. There is
   no Rust, no TypeScript validation code, and zero matches for `ODCS`/`ODPS` in
   any `.rs`/`.ts`/`.py`/`.toml` file. Any assumption that a validator lives here
   is wrong.
2. **The ODCS/ODPS "validators" appear to be schema assets plus SDK
   field-preservation logic, not standalone validators with their own diagnostic
   types and CLIs.** `data-modelling-sdk/src` returned no matches for `valid`.
   The vendored schemas and the `003-odcs-field-preservation` spec suggest
   validation is currently JSON-Schema-driven inside the SDK rather than a
   first-class validator.
3. **Consequence for the plan.** Spec Scenario 2, FR-010 and SC-001 assume a
   *pre-migration validator output* to differentially test against. If no such
   validator exists for ODCS/ODPS, then that work is **greenfield construction,
   not migration**, and the "differential test against pre-migration output"
   acceptance criterion is unsatisfiable as written for those two formats.

   Only **OKF** has a genuine migration path with a real regression oracle
   (`tests/okf_interop.rs` + vendored upstream fixtures). This independently
   confirms the spec's own OQ-002 recommendation to **do OKF first**, and
   strengthens it from a preference into a constraint.

**Action required:** before Phase 3 begins, either (a) confirm ODCS/ODPS
validators exist somewhere not yet inspected, or (b) amend the ADR and spec to
describe ODCS/ODPS as new-build adapters with schema-conformance fixtures as
their oracle instead of differential tests.

---

## 2. Crate architecture

Nine crates, each independently versioned and separately publishable. The
dependency graph is strictly acyclic and `conform-core` sits at the root with no
format knowledge and no runtime dependencies.

```
                       ┌──────────────────┐
                       │   conform-core   │  0.1.0   zero non-std deps
                       │ Diagnostic       │
                       │ Severity         │
                       │ Location         │
                       │ ConformanceReport│
                       │ Validator trait  │
                       │ Reference resol. │
                       │ report-vs-gate   │
                       └────────┬─────────┘
             ┌──────────────┬───┴────┬──────────────┬─────────────┐
             │              │        │              │             │
      ┌──────▼─────┐ ┌──────▼─────┐ ┌▼───────────┐ ┌▼──────────┐ ┌▼──────────┐
      │conform-odcs│ │conform-odps│ │conform-okf │ │conform-   │ │conform-   │
      │   0.1.0    │ │   0.1.0    │ │   0.1.0    │ │lexicon    │ │registry   │
      └──────┬─────┘ └──────┬─────┘ └─────┬──────┘ │  0.1.0    │ │  0.1.0    │
             │              │             │        └─────┬─────┘ └─────┬─────┘
             └──────────────┴─────┬───────┴──────────────┘             │
                                  │                                    │
                         ┌────────▼────────────────────────────────────▼──┐
                         │            conform-cli  (bin: `conform`)       │  0.1.0
                         │      TUI (ratatui) + --json + batch verify     │
                         └────────────────────────────────────────────────┘

      ┌──────────────┐        ┌──────────────┐
      │ conform-ffi  │ 0.1.0  │ conform-web  │ 0.1.0  (SPA, not published to crates.io)
      │ C ABI cdylib │        │              │
      └──────────────┘        └──────────────┘
```

### 2.1 `conform-core` — the harness

Satisfies FR-001 … FR-008. Public surface:

```rust
pub enum Severity { Error, Warning, Info }

pub struct Location {
    pub document: DocumentId,      // opaque, format-agnostic
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub pointer: Option<Pointer>,  // JSON Pointer / field path
}

pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,      // stable, machine-readable, e.g. "ODCS001"
    pub message: String,
    pub location: Location,
    pub help: Option<String>,
    pub spec_ref: Option<SpecRef>, // ← links a diagnostic to the registry entry
}

pub struct ConformanceReport { /* … */ }
impl ConformanceReport {
    pub fn diagnostics(&self) -> &[Diagnostic];
    pub fn by_severity(&self, s: Severity) -> impl Iterator<Item = &Diagnostic>;
    pub fn should_gate(&self, policy: GatePolicy) -> bool;   // FR-003, FR-006
}

pub trait Validator {
    type Document;
    fn validate(&self, doc: &Self::Document) -> ConformanceReport;
    fn spec(&self) -> SpecRef;     // which registry entry this validates against
}

pub enum Resolution {             // FR-005 — the three-way result
    Resolves(Target),
    DoesNotExist,
    NotInspected { reason: NotInspectedReason },
}
```

Two deliberate additions beyond the spec:

- **`spec_ref` on `Diagnostic`** — every diagnostic names the registry entry (and
  therefore the upstream spec version) it was raised under. This is what lets the
  TUI and SPA answer "which version of ODCS said this was wrong?" and is the
  thread that ties the registry to the validator output.
- **`GatePolicy`** — makes FR-006's report-vs-gate split an explicit parameter
  rather than a convention each CLI remembers.

**Dependency discipline (NFR-001):** zero non-`std` dependencies by default.
`serde` is an *optional feature* (`features = ["serde"]`), off by default, so a
consumer that only wants the trait surface pays nothing. This keeps
`cargo deny --all-features check` trivially clean.

**Guard for NFR-002 (no format leakage):** a CI test greps the crate's own source
for the strings `odcs`, `odps`, `okf`, `lexicon`, `yaml`, `frontmatter`
(case-insensitive) and fails the build on a hit. The spec says format leakage is
"a defect, not a style note" — this makes that enforceable rather than aspirational.

### 2.2 Format adapter crates

`conform-odcs`, `conform-odps`, `conform-okf`, `conform-lexicon`. Each owns its
schema model, parser and rules, implements `Validator`, and depends on
`conform-core` + `conform-registry`. Each is independently versioned because each
tracks a *different upstream spec on a different release cadence* — the single
strongest argument for per-crate versions, and a direct extension of the
`rto-okf-syntax` reasoning.

`conform-okf` is a thin adapter over Roteiro's existing `rto-okf-syntax`
(already `0.1.x` and already designed to be given away) rather than a reimplementation.

### 2.3 `conform-registry` — upstream canonical source tracking

The crate that answers your requirement directly. Full design in §3.

### 2.4 `conform-cli` — TUI + JSON

Binary `conform`. Full design in §4.

### 2.5 `conform-ffi` — optional FFI

Full design in §5. Not a dependency of anything else; nothing depends on it.

### 2.6 `conform-web` — SPA

Full design in §6. Published to a static host, not crates.io.

---

## 3. The spec registry — tracking upstream canonical sources

### 3.1 Problem being solved

Today: `odps-json-schema-latest.json` sits vendored with no version, no source
URL, no fetch date. Upstream URLs live as hand-typed hyperlinks in ~20 Markdown
files. Nothing can tell you that Bitol published ODCS v3.2.0 last week.

### 3.2 `specs.toml` — the source of truth

One file at the repo root, modelled on the union of Roteiro's `PROVENANCE.md`
(prose provenance) and SchemaStore's `catalog.json` (machine-readable catalogue).

```toml
schema_version = 1

[[spec]]
id            = "odcs"
name          = "Open Data Contract Standard"
abbreviation  = "ODCS"
version       = "3.1.0"
status        = "current"          # current | superseded | draft | deprecated

  [spec.upstream]
  homepage    = "https://bitol-io.github.io/open-data-contract-standard/latest/"
  repository  = "https://github.com/bitol-io/open-data-contract-standard"
  steward     = "Bitol (LF AI & Data)"
  steward_url = "https://bitol.io/"
  licence     = "Apache-2.0"

  [spec.artefact]
  kind        = "json-schema"
  upstream_path = "schemas/odcs-json-schema-v3.1.0.json"
  local_path  = "schemas/odcs-json-schema-v3.1.0.json"
  sha256      = "…"                # of the vendored bytes
  fetched_at  = "2026-09-20"
  pinned_ref  = "v3.1.0"           # tag or commit SHA — never "latest"

  [spec.update_check]
  method      = "github-release"   # github-release | github-tag | http-etag | manual
  endpoint    = "https://api.github.com/repos/bitol-io/open-data-contract-standard/releases/latest"
  field       = "tag_name"
  cadence     = "weekly"

[[spec]]
id           = "okf"
name         = "Open Knowledge Format"
version      = "0.2"
  [spec.upstream]
  repository = "https://github.com/GoogleCloudPlatform/open-knowledge-format"
  licence    = "Apache-2.0"
  copyright  = "Google LLC"
  [spec.artefact]
  pinned_ref = "ad30107c31c06aec8a7d5636e0d1058118604e6f"
  fetched_at = "2026-08-21"
  [spec.update_check]
  method     = "github-commit"
  endpoint   = "https://api.github.com/repos/GoogleCloudPlatform/open-knowledge-format/commits/main"
  field      = "sha"
```

Design rules:

- **`pinned_ref` is mandatory and may never be `latest`.** A schema-validation
  test on `specs.toml` rejects the literal string `latest`. This makes the
  `odps-json-schema-latest.json` defect structurally impossible to reintroduce.
- **`sha256` of vendored bytes** lets `conform registry verify` prove the local
  copy has not drifted or been hand-edited.
- **`spec_ref`** on every `Diagnostic` points at a `spec.id` + `version`.

### 3.3 Drift detection

`conform registry check` — polls each `update_check.endpoint`, compares to
`pinned_ref`, reports one of `up-to-date` / `upstream-newer` / `unreachable`.

- Runs as a **scheduled weekly CI job**, not on every build — an upstream release
  must never break an unrelated PR. It opens an issue, it does not fail a gate.
  This is a direct application of the spec's own report-vs-gate lesson: a network
  check that gates is a check that will cry wolf and lose trust, exactly as the
  `links --check` incident did.
- `--offline` skips all network access; the default for local dev and for
  `cargo test`.

### 3.4 Migration of existing vendored schemas

Phase 2 deliverable: write `specs.toml` entries for all four vendored schemas in
`data-modelling-sdk/schemas/`, resolve what `odps-json-schema-latest.json`
actually is (diff against Bitol releases to identify the version), pin it, and
de-duplicate the copy under `specs/003-odcs-field-preservation/schemas/`.

---

## 4. TUI console + `--json`

Binary: `conform`. Crate: `conform-cli`. Stack: `ratatui` + `crossterm` —
greenfield, since nothing in the estate uses either today.

### 4.1 Command surface

```
conform                                 # launch interactive TUI
conform list                            # list registered specs + upstream links
conform validate <path>                 # report; exit 0 even with findings
conform validate <path> --check         # gate; non-zero exit on error severity
conform validate <path> --json          # machine-readable, full detail
conform validate <path> --spec odcs     # restrict to one spec
conform registry check                  # upstream drift report
conform registry verify                 # vendored-bytes checksum verification
```

Invariants inherited from `conform-core`, enforced by tests:

- **`--json` changes serialisation only** (FR-007). A test asserts the diagnostic
  set and exit code are byte-identical with and without `--json`.
- **Bare `validate` reports and exits 0**; only `--check` gates (FR-006,
  Scenario 5).

### 4.2 TUI layout

Three panes, `spec list → document list → diagnostic detail`:

```
┌─ Specs ─────────────┬─ Documents ──────────────┬─ Diagnostic ─────────────────┐
│ ▸ ODCS    v3.1.0  ✓ │  contracts/orders.yaml ✗ │ ODCS003  error               │
│   ODPS    v1.0.0  ⚠ │  contracts/users.yaml  ✓ │ servers[0].host is required  │
│   OKF     v0.2    ✓ │  contracts/events.yaml ⚠ │                              │
│   Lexicon v0.1    ✓ │                          │ orders.yaml:14:5             │
│                     │                          │ pointer: /servers/0/host     │
│ [u] upstream ▸      │                          │                              │
│ ODCS v3.2.0         │                          │ spec: ODCS v3.1.0            │
│ available upstream  │                          │ ▸ bitol-io.github.io/…       │
└─────────────────────┴──────────────────────────┴──────────────────────────────┘
 [tab] pane  [/] filter  [c] check  [j] json  [u] upstream  [o] open link  [q] quit
```

Requirements this satisfies:

- **Navigation of specs** — left pane is the `specs.toml` registry, showing
  version, conformance status, and an upstream-newer indicator (`⚠`).
- **Verification** — `[c]` runs validation over the selected document set.
- **Upstream visibility** — `[u]` shows the canonical source; `[o]` opens it in a
  browser. The upstream link is never more than two keystrokes away, satisfying
  "list them so users can see the link."
- **Detail** — the right pane is the full diagnostic: code, severity, message,
  location with line/column *and* JSON pointer, help text, and the spec version
  under which it was raised.

Accessibility: no colour-only encoding — every severity carries a glyph
(`✗ ⚠ ✓`); `NO_COLOR` honoured; degrades to plain output when not a TTY.

### 4.3 `--json` envelope

Stable, versioned, one object per run:

```json
{
  "schema_version": 1,
  "tool": { "name": "conform", "version": "0.1.0" },
  "summary": { "documents": 3, "error": 1, "warning": 1, "info": 0, "gated": false },
  "specs": [
    { "id": "odcs", "version": "3.1.0",
      "upstream": { "homepage": "https://bitol-io.github.io/open-data-contract-standard/latest/",
                    "repository": "https://github.com/bitol-io/open-data-contract-standard",
                    "pinned_ref": "v3.1.0", "upstream_newer": null } }
  ],
  "diagnostics": [
    {
      "severity": "error",
      "code": "ODCS003",
      "message": "servers[0].host is required",
      "help": "Add a `host` key, or remove the empty server entry.",
      "location": { "document": "contracts/orders.yaml", "line": 14, "column": 5,
                    "pointer": "/servers/0/host" },
      "spec_ref": { "id": "odcs", "version": "3.1.0" }
    }
  ]
}
```

`--json` is **per-spec addressable** via `--spec <id>`, so CI can gate one format
at a time. The envelope carries `schema_version` from day one so downstream
consumers (the SPA, badge logic, FR-019 in the lexicon spec) can version-negotiate.

---

## 5. Optional FFI layer

Crate `conform-ffi`, **entirely optional and entirely additive** — no other crate
depends on it, and its absence changes nothing.

### 5.1 Design

- **Core:** a C ABI (`crate-type = ["cdylib", "staticlib", "rlib"]`) with
  `cbindgen`-generated `conform.h`.
- **Surface kept deliberately tiny** — a handle-based API, opaque pointers, no
  Rust types crossing the boundary:

```c
conform_ctx*  conform_new(const char* registry_toml_path);
int           conform_validate(conform_ctx*, const char* spec_id,
                               const char* document_path,
                               char** out_json, size_t* out_len);
void          conform_string_free(char* s);
void          conform_free(conform_ctx*);
const char*   conform_last_error(conform_ctx*);
```

- **The boundary speaks JSON, not structs.** Every result crosses as the §4.3
  envelope. This means the ABI surface is ~6 functions that never change shape
  when a `Diagnostic` field is added — the JSON schema absorbs that evolution.
  It is the single most important decision here: a struct-passing FFI would make
  every `conform-core` change an ABI break across four language bindings.
- **Panic discipline:** every entry point wrapped in `catch_unwind`; a panic
  becomes an error code, never an unwind across the FFI boundary (UB).
- **Thread safety:** `conform_ctx` is `Send`, documented as not `Sync`; one
  context per thread.

### 5.2 Language bindings (each independently feature-gated, each its own phase)

| Binding | Mechanism | Feature | Phase |
|---|---|---|---|
| C / C++ | cbindgen header | default | 6 |
| Python | `pyo3` + `maturin` wheel | `python` | 6 (stretch) |
| Node | `napi-rs` | `node` | deferred |
| WASM | `wasm-bindgen` | `wasm` | 7 — **needed by the SPA** |

The WASM binding is not speculative: it is what lets the SPA (§6) validate a
pasted document in-browser with zero backend. That makes the FFI layer *load
bearing for the SPA demo*, which is the strongest justification for building it.

### 5.3 Why "optional" is enforced, not just stated

`cargo deny` and the dependency-discipline test run against `conform-core`
**without** `--features ffi`. The FFI crate's dependencies (`cbindgen`, `pyo3`,
`wasm-bindgen`) are heavy and MUST NOT leak into the harness's tree — the exact
failure mode NFR-001 was written to prevent.

---

## 6. Single-page app

Crate/directory `conform-web`. Not published to crates.io; deployed as static
assets.

### 6.1 Purpose

Three jobs, in priority order:

1. **Describe the tool** — what Open Spec Conform is, the crate family, how to
   adopt it, the `Validator` trait contract (SC-004 says a newcomer must be able
   to prototype a fifth format from published docs alone — this is where those
   docs live).
2. **List the specs and their canonical upstream sources** — generated *from
   `specs.toml`*, never hand-typed. Each card shows name, version, steward,
   licence, pinned ref, fetch date, a direct link to the canonical upstream, and
   a freshness badge (`current` / `upstream newer` / `unverified`).
3. **Live validation demo** — paste or drop a document, validate it in-browser
   via the WASM binding (§5.2), see the §4.3 diagnostics rendered.

### 6.2 Stack decision

**Recommendation: a static site generated at build time, with a small WASM
island for the demo.** Concretely: Astro (or plain Vite + TypeScript) producing
static HTML, plus the `conform-ffi` WASM bundle loaded only on the demo route.

Rationale, and the one place this plan deliberately diverges from house
precedent: `open-data-modelling` already uses **Hugo**, and reusing it would be
the consistent choice. But the spec-catalogue page must be generated from
`specs.toml`, and the demo needs to load a WASM module — both are awkward in Hugo
and native to Astro/Vite. The catalogue generation is the deciding factor: it is
the difference between the ~20 hand-typed upstream links that exist today and a
single source of truth.

**This is an open decision — see OQ-104.** If cross-site consistency with the
existing Hugo estate matters more than generation ergonomics, Hugo with a data
file is a legitimate alternative and the plan survives the swap.

### 6.3 Content structure

```
/                     What Open Spec Conform is; the problem; the crate family
/specs                Catalogue generated from specs.toml (the upstream-source list)
/specs/:id            One spec: version history, upstream links, rules it enforces
/try                  WASM live validation demo
/docs/adopting        Implementing Validator for a new format (SC-004)
/docs/cli             conform CLI + TUI reference, --json envelope schema
```

---

## 7. Workspace, versioning and release

### 7.1 Repository

Standalone repo `OffeneDatenmodellierung/open-spec-conform` — resolving **OQ-001**
in favour of the spec's own recommendation. Justification is now stronger than
when the spec was written: the crates are consumed by at least three otherwise
independent repos (`Roteiro`, `data-modelling-sdk`, `data-modelling-api`), and
vendoring it into any one of them would couple the other two to that repo's
release cadence.

### 7.2 Versioning — independently publishable crates

Every crate carries a **literal version in its own manifest**. No
`version.workspace = true` anywhere. This inverts Roteiro's default and matches
its `rto-okf-syntax` exception, for the same reason: these crates are *not* one
product cut into pieces. `conform-core` and `conform-odcs` track different
upstreams on different cadences and must be pinnable separately (FR-011).

`release-plz.toml`:

```toml
[workspace]
changelog_update   = true
git_release_enable = true
dependencies_update = false
semver_check       = true      # ← ENABLED, unlike Roteiro
```

`semver_check = true` is the deliberate difference from Roteiro. Roteiro disables
it *because* its crates share one version, which makes `cargo-semver-checks` force
spurious workspace-wide majors. With per-crate versions that pathology disappears,
and since four internal consumers will pin against `conform-core` immediately,
semver enforcement is exactly what we want — **resolving OQ-003 in favour of
strict semver from the first release.**

### 7.3 Conventions inherited from Roteiro

`edition = "2024"`, `rust-version = "1.96"`, `license = "MIT OR Apache-2.0"` with
both licence files, `rustfmt.toml` (`edition = "2024"`), `clippy.toml`, and
`deny.toml` run with `--all-features` in CI — Roteiro's `deny.toml` notes that
widening to `--all-features` is what closed its issue #318, since anything
reachable only through optional features was previously invisible to both the
licence and advisory gates. We start there rather than relearning it.

### 7.4 CI gates

| Gate | Scope |
|---|---|
| `cargo test --all-features` | workspace |
| `cargo clippy --all-targets --all-features -- -D warnings` | workspace |
| `cargo fmt --check` | workspace |
| `cargo deny --all-features check` | workspace + `conform-core` alone |
| no-format-leakage grep | `conform-core` only (NFR-002) |
| `specs.toml` schema + no-`latest` check | registry |
| per-file coverage ratchet | workspace |
| upstream drift check | **scheduled weekly, non-gating** |

---

## 8. Phased delivery

Each phase is one or more PRs, each independently reviewable. Phases 1–2 are
prerequisites for everything; 3–7 can then proceed with some parallelism.

| Phase | Deliverable | Depends on | Exit criteria |
|---|---|---|---|
| **1** | `conform-core` 0.1.0 — types, trait, resolution, gate policy | — | Compiles with zero non-std deps; `cargo deny` clean standalone; no-leakage gate passes; three-way `Resolution` explicitly tested for "exists but not inspected" vs "does not exist" |
| **2** | `conform-registry` 0.1.0 + `specs.toml` for all known specs | 1 | Every vendored schema in `data-modelling-sdk` has a pinned entry; `odps-…-latest.json` identified and pinned; `registry verify` green; `latest` rejected by test |
| **3** | `conform-okf` 0.1.0 — migrate Roteiro's `okf::conform` | 1, 2 | `tests/okf_interop.rs` passes **unchanged in its assertions**; `Finding`/`CheckReport` retired; CLI behaviour byte-identical (NFR-005) |
| **4** | `conform-cli` 0.1.0 — CLI, `--json`, then TUI | 1, 2, 3 | `--json`/non-`--json` diagnostic-set equality test; bare `validate` exits 0 with findings; TUI navigates registry and shows upstream links |
| **5** | `conform-odcs` + `conform-odps` 0.1.0 | 1, 2, §1.4 resolved | Schema-conformance fixtures pass; **oracle strategy agreed per §1.4** |
| **6** | `conform-ffi` 0.1.0 — C ABI + header (+ Python stretch) | 1 | `catch_unwind` at every boundary; valgrind/ASan clean on the C smoke test; `conform-core` tree unchanged by its existence |
| **7** | `conform-web` — SPA + WASM demo | 2, 6 | `/specs` generated from `specs.toml` with zero hand-typed URLs; demo validates in-browser |
| **8** | `conform-lexicon` | 1, 2 | Zero bespoke diagnostic/report/gate types (SC-002) |

**Critical path: 1 → 2 → 3.** Phase 3 is the highest-information phase — it is the
only one with a real regression oracle, so it is where a wrong abstraction in
Phase 1 will surface. Do not start Phase 5 before Phase 3 completes; if
`conform-core`'s shape is wrong, OKF will prove it cheaply and ODCS/ODPS will not.

---

## 9. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | ODCS/ODPS validators do not exist, so FR-010's differential testing is unsatisfiable | §1.4 — resolve before Phase 5; amend ADR if confirmed |
| R2 | `conform-core` abstraction is wrong and only discovered at the fourth adapter | Phase 3 first (real oracle); treat 0.1.x as unstable; `semver_check` catches breaks |
| R3 | Format leakage into `conform-core` | Automated grep gate (§2.1), not code review alone |
| R4 | FFI dependencies leak into the harness tree | `cargo deny` run on `conform-core` in isolation (§5.3) |
| R5 | Upstream drift check becomes a flaky network gate that erodes trust | Non-gating scheduled job, `--offline` default (§3.3) — the explicit lesson of the `links --check` incident |
| R6 | Nine crates is a lot of release overhead for a small team | `release-plz` automation; crates 6–8 are optional and can slip without blocking 1–5 |
| R7 | SPA stack diverges from the Hugo estate | OQ-104 — decide deliberately, not by default |

---

## 10. Open questions

Carried forward from the spec, plus new ones raised by this plan.

- **OQ-001 — repository location.** *Resolved:* standalone repo (§7.1).
- **OQ-002 — migration order.** *Resolved:* OKF first, now a constraint rather
  than a preference (§1.4).
- **OQ-003 — strict semver from first release.** *Resolved:* yes, enabled by
  per-crate versioning (§7.2).
- **OQ-101 — Do ODCS/ODPS validators exist?** **Blocking for Phase 5.** §1.4.
- **OQ-102 — Does `conform-okf` wrap `rto-okf-syntax` or absorb it?** The latter
  would let Roteiro delete a crate; the former is lower risk. Recommend wrap first.
- **OQ-103 — Is `lexicon-core` in this repo or the MDM lexicon repo?** The spec
  implies the latter; the dependency direction works either way.
- **OQ-104 — SPA stack: Astro/Vite or Hugo?** §6.2. Trade generation ergonomics
  against estate consistency.
- **OQ-105 — Which FFI bindings are actually wanted?** C + WASM are justified
  (WASM is required by the SPA). Python and Node are speculative — do not build
  them without a named consumer.

---

## 11. What to review first

1. **§1.4** — the ADR premise corrections. Everything in Phase 5 depends on this.
2. **§7.2** — per-crate versioning with `semver_check = true`, which deliberately
   inverts Roteiro's workspace default.
3. **§6.2** — the SPA stack divergence from the existing Hugo site.
4. **§5.1** — the JSON-over-FFI decision, which trades a little performance for an
   ABI that survives `Diagnostic` evolution.
