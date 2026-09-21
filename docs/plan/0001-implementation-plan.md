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

Rust, `crates/` layout, edition 2024, MIT, workspace version `2.4.0` — with
`data-modelling-core` **already independently versioned at `2.0.6`**, a second
precedent for the per-crate versioning this plan requires (§7.2).

Three members: `crates/core` (`data-modelling-core`), `crates/odm` (the CLI),
`crates/wasm` (`data-modelling-wasm` 2.4.0, `wasm-bindgen 0.2`).

**Validators exist.** `crates/core/src/validation/` contains `input.rs`,
`mod.rs`, `relationships.rs`, `schema.rs`, `tables.rs`, `xml.rs`, and
`schema.rs` exports eighteen validator entry points:

```rust
pub fn validate_odcs_internal(content: &str) -> Result<(), String>   // schema.rs:33
pub fn validate_odps_internal(content: &str) -> Result<(), String>   // schema.rs:208
```

…and the same shape for `odcl`, `cads`, `openapi`, `protobuf`, `avro`,
`json_schema`, `sql`, `workspace`, `relationships`, `decision`, `knowledge`,
`decisions_index`, `knowledge_index`, `sketch`, `sketch_index`, `dbmv`.

`crates/odm/src/commands/validate.rs` wires them to a CLI:
`handle_validate(format, input)` matches a format string and reads from a path
or stdin via `-`.

**Two structural defects make this the motivating case for `conform-core`:**

1. **`Result<(), String>` discards everything a diagnostic needs.** One
   stringly-typed error per document: no severity, no stable code, no location,
   no way to report a second problem. A contract with twelve issues reports one
   string. Nothing downstream can filter, group, sort, count by severity, or
   link a finding to a spec clause — which is precisely the payload
   `conform-core`'s `Diagnostic`/`Severity`/`Location`/`ConformanceReport`
   exist to carry.
2. **Feature-gated no-op stubs silently pass.** Every schema-backed validator is
   duplicated under `#[cfg(not(feature = "schema-validation"))]`
   (e.g. `schema.rs:80` for ODCS, `schema.rs:243` for ODPS) returning `Ok(())`
   without inspecting the content. Build without the feature and validation
   silently becomes a no-op that reports success. §5.3's "optional means
   enforced, not stated" rule exists because of exactly this pattern.

**Reference resolution already exists** in `crates/odm/src/reference.rs` —
`resolve_local_reference`, `resolve_http_reference`, and a
`resolve_reference(reference, source_file)` front door, with the HTTP arm
itself cfg-stubbed (`reference.rs:84`). This is the direct precedent for
`conform-core`'s three-way `Resolution`, and it should be lifted rather than
reinvented.

**Spec-driven development is the house process.** `crates/core/specs/` holds
`001-odps-validation`, `001-wasm-exports`, `002-table-metadata`,
`003-odcs-field-preservation`, `004-bpmn-dmn-openapi`,
`005-databricks-sql-support`, `006-cli-wrapper` — numbered spec directories,
the same convention this repo uses.

A top-level `schemas/` directory holds the **vendored JSON Schemas**:

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

#### 1.2.1 Verified provenance of the two vendored Bitol schemas

Measured directly, not inferred. Both files are dated 15 Aug 05:40.

| | `odcs-json-schema-v3.1.0.json` | `odps-json-schema-latest.json` |
|---|---|---|
| size | 86 441 bytes | 15 783 bytes |
| SHA-256 | `2cb7dd6fe43344d2233e0406438622681dc3ebadcf8f0d606a15b40c8f6752c0` | `95ae53a90d85d17666b6a28d22ea5fff20313048fded5b92599546efe0589e14` |
| `$schema` | draft 2019-09 | draft 2019-09 |
| `$id` | **absent** | **absent** |
| `title` | `Open Data Contract Standard (ODCS)` | `Open Data Product Standard (ODPS)` |

Two findings, both of which strengthen the case for `conform-registry`:

**Neither file carries a `$id`.** A JSON Schema's `$id` is the one field that
would record where the document came from. With it absent, and with no
`PROVENANCE.md`, the vendored bytes contain *zero* machine-readable link back to
Bitol. The filename is the only provenance signal that exists, which is precisely
why `odps-json-schema-latest.json` is unanswerable — its filename encodes nothing.

**The ODPS version is nonetheless recoverable, from `properties.apiVersion`:**

```json
"apiVersion": { "default": "v1.0.0", "enum": ["v0.9.0", "v1.0.0"] }
```

A schema can only enumerate `apiVersion` values that existed when it was written,
so the enum's maximum dates the document. **`odps-json-schema-latest.json` is
ODPS v1.0.0** (accepting v0.9.0 documents). The same test applied to the ODCS file
gives `enum: [v3.1.0, v3.0.2, v3.0.1, v3.0.0, v2.2.2, v2.2.1, v2.2.0]`, max
`v3.1.0`, corroborating its filename.

This resolves the Phase 2 unknown ahead of schedule: the ODPS entry pins to
`v1.0.0`, and the file should be renamed `odps-json-schema-v1.0.0.json` to match.
Note the inference is *dating*, not proof of byte-identity with any upstream tag —
Phase 2 must still diff the vendored bytes against Bitol's `v1.0.0` artefact and
record the result, since a local edit would not show up in the enum.

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
validator."* **The premise holds.** An earlier draft of this plan said it did
not; that draft was wrong because it searched `data-modelling-sdk/src`, a path
that does not exist — the code is under `crates/core/src/`. The corrected
findings are in §1.2 and they change the shape of the work in three ways.

1. **`open-data-modelling` really is a Hugo documentation site, not a
   validator.** `hugo-site/`, `specs/`, `package.json`, Markdown. No Rust, no
   TypeScript validation code. This part of the earlier correction stands: no
   validator lives here, and the ADR's "ODM's validator" means
   `data-modelling-sdk`, not `open-data-modelling`.

2. **The migration oracle exists after all, for ODCS *and* ODPS.** Eighteen
   `validate_*_internal` functions with a stable signature and a CLI on top of
   them (§1.2) are exactly the *pre-migration validator output* that Spec
   Scenario 2, FR-010 and SC-001 require. Differential testing is **satisfiable
   as written**: drive both the old `Result<(), String>` function and the new
   `conform-*` adapter over the same fixture corpus and assert the new one
   flags a superset. Risk R1 is retired.

3. **The value proposition is stronger than "migration", and it folds back.**
   These validators are not missing; they are *lossy*. They collapse an
   arbitrary number of findings into one `String` and silently degrade to
   `Ok(())` when a cargo feature is off. So `conform-core` is not re-plumbing
   working code for tidiness — it recovers diagnostic information that is being
   destroyed today, and every consumer of `data-modelling-core` gains
   multi-diagnostic, severity-aware, located output the moment the adapter
   lands. That is the "fold back into the other projects" return, and it
   argues for doing ODCS/ODPS **early**, not last.

**Consequences for sequencing.** OKF keeps its head start on quality of oracle
(vendored upstream fixtures pinned in `PROVENANCE.md`), so Phase 3 still does
OKF first — but for the narrower reason that its fixture corpus is the best,
not because ODCS/ODPS lack an oracle. This paragraph used to cite
`tests/okf_interop.rs` as half of that oracle; it is not one — see §8.1.
Phase 5 is unblocked and its acceptance criterion is unchanged.

**Consequences for scope.** The estate validates far more than ODCS/ODPS/OKF:
ODCL, CADS, OpenAPI, Protobuf, Avro, JSON Schema, SQL, DBMV, plus first-party
`workspace`/`decision`/`knowledge`/`sketch` documents. `conform-core`'s
`Validator` trait and the `specs.toml` registry must therefore be sized for
**~18 formats, not 4**. §2.2 and §3.2 are written accordingly; the four named
adapter crates are the first wave, not the whole set.

**Two defects to fix on the way through**, both from §1.2 and both worth
raising upstream independently of this workstream: the `odps-json-schema-latest.json`
unpinned vendored schema, and the `cfg(not(feature = "schema-validation"))`
stubs that report success without validating.

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
`data-modelling-sdk/schemas/`, and de-duplicate the copy under
`specs/003-odcs-field-preservation/schemas/`.

The ODPS version question is **already answered** (§1.2.1): the file is ODPS
v1.0.0, dated by its `apiVersion` enum. Phase 2 therefore reduces to confirming
byte-equivalence against Bitol's published v1.0.0 artefact — the enum dates the
document but cannot detect a local modification — then pinning it and renaming
the file to `odps-json-schema-v1.0.0.json`.

If that diff comes back non-identical, that is a *finding, not a blocker*: record
the divergence in the entry's `notes` field and pin the SHA-256 regardless. A
knowingly-divergent vendored copy with recorded provenance is strictly better
than today's situation, where divergence would be invisible.

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

### 5.4 Correction — the ABI as built

§5.1's C sketch above was written before the crate existed and is **wrong about
the signatures**, though right about everything it was actually arguing for: the
JSON boundary, the opaque handle, the `catch_unwind` discipline and the `Send`
but not `Sync` rule all survived intact. The sketch is left in place because the
reasoning around it is still the reasoning; this section records what was
actually shipped in Phase 6 and why each difference exists. A plan whose code
sample no longer compiles is a plan people stop reading.

```c
const char*        conform_version(void);
ConformValidator*  conform_validator_new(const char *spec_id,
                                         const char *registry_path);
enum ConformStatus conform_validate(ConformValidator *validator,
                                    const char *document_id,
                                    const uint8_t *document, size_t document_len,
                                    char **out_json, size_t *out_len);
const char*        conform_last_error(void);
void               conform_string_free(char *text);
void               conform_validator_free(ConformValidator *validator);
enum ConformStatus conform_self_test_panic(void);
```

**Bytes, not a path.** The sketch's `conform_validate` took a
`const char* document_path`. An embedder rarely has one: the document is in a
request body, an editor buffer, a socket, a database column. A path-taking
boundary makes every such caller write a temporary file and clean it up, to be
read straight back by a library in the same process. The boundary takes a
pointer and a length; a caller who *does* have a path reads the file, which is
the easy direction.

**The handle is per specification, not per registry.** The sketch's
`conform_ctx` held a registry and took a `spec_id` on every call. The expensive
thing here is compiling a JSON Schema, so that context would have to either
recompile per document or cache invisibly, and the provenance check — the
re-hash of the vendored bytes against `specs.toml` — would happen somewhere the
caller could not see. Binding the schema to the handle makes both explicit:
`conform_validator_new` is where the registry is read, where the digest is
verified, and where a drifted schema is refused. What comes back is a validator
that has already earned the right to issue verdicts.

**`conform_last_error` takes no argument.** It could not take a context: the
call most likely to fail is the one that *creates* the handle, and it has no
handle to leave a message on. The slot is thread-local, with `strerror`'s
lifetime contract, which also means two threads failing at once do not overwrite
each other's explanation.

**The return type is an enum, not an `int`.** `ConformStatus` is `#[repr(i32)]`
and cbindgen emits it with its values, so a C caller switches on names. `0` is
success and nothing else is, so a binding that does not recognise a future code
still gets the question right.

**Two entry points the sketch did not have.** `conform_version()` reports the
library's release. `conform_self_test_panic()` panics on purpose, catches it,
and returns `CONFORM_STATUS_PANIC` — because the panic discipline has a
prerequisite the linker does not check: a `panic = "abort"` build has no
unwinding to catch, and `catch_unwind` cannot help. A binding calls it once at
start-up; a process that dies there has linked a build in which no entry point
is safe to call from C. Two compile-time constants come with them,
`CONFORM_ABI_VERSION` and `CONFORM_SCHEMA_VERSION`, because "can I link this",
"which release is this" and "can I parse this report" are three questions and
the sketch had a number for none of them.

**`okf` is not reachable across this boundary.** An OKF bundle is a directory of
cross-referencing files and `conform_okf::load` takes a filesystem root; a
bytes-in boundary has nothing to hand it. Inventing an archive format for a
directory is a much larger decision than an FFI should make on its own, so
asking for `okf` fails with a message saying exactly that, rather than returning
an empty report. `odcs` and `odps` are reachable. If the SPA's WASM demo (§6)
needs bundles, that is the phase where the encoding question gets answered
properly.

**cbindgen is outside the workspace.** §5.3 says the FFI crate's build
dependencies must not leak into the tree, and the licence gate agreed more
forcefully than expected: `cbindgen` is MPL-2.0 and `deny.toml` allows
permissive licences only, so `cargo deny --all-features check` rejects it as a
dependency of `conform-ffi` however optional the feature. The generator
therefore lives in `tools/headergen`, its own cargo root, exactly as
`tools/oracle` does — and the header it produces is committed, with a test that
regenerates and compares. `deny.toml` was not touched.

**On §7.2's "valgrind/ASan clean".** Those are two claims about two tools with
two availabilities. valgrind is not packaged for Apple silicon, so on the
machine Phase 6 was written on it could not be run at all; AddressSanitizer
could, and was clean, but LeakSanitizer is inactive on that platform — a
deliberate leak goes unreported there even with `detect_leaks=1`. So a macOS
developer gets the invalid-access half and not the unreleased-memory half.
`tools/sanitise/run.sh` runs what is present, builds two programs that are
*supposed* to fail so that a clean run cannot be a checker that silently did not
run, and exits non-zero if nothing ran. Both arms have since been run on Linux —
valgrind clean, ASan clean, both controls caught — and both halves of the
criterion are now met. See `tools/sanitise/README.md` for what was measured on
which host, and for the two defects the first version of that harness had.

**On the panic net, and the one place it does not exist.** §5.1's "a panic
becomes an error code, never an unwind across the FFI boundary" holds only on a
target that unwinds. Phase 7 built `conform-ffi` for `wasm32-unknown-unknown`
and called it from Node: `conform_version()` returned `"0.1.0"`, so the ABI is
sound there, and `conform_self_test_panic()` trapped with `unreachable`, because
that target is `panic = "abort"` and there was nothing to catch. That is the
self-test doing exactly its job — it fired on the first real target and stopped a
demo shipping on an assumption that was false there. Phase 7 must therefore treat
a panic in the WASM binding as fatal to the instance and design around it, rather
than inheriting a guarantee the C build has and it does not.

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

### 6.4 Correction — the SPA as built, and the WASM question resolved

§6.2 recommended Astro or Vite, and §6.3 sketched six routes. Phase 7 built
neither, and this section records what was shipped and why each difference
exists — as §5.4 does for the FFI. The plan's *reasoning* survives intact; its
stack recommendation did not, and §6.2 flagged itself as an open decision
(OQ-104), so this resolves it.

**One page, not six routes.** The request this phase was built from asks for "a
single page app that describes the tool, [the] crates and the specs". `/specs`,
`/specs/:id`, `/docs/adopting` and `/docs/cli` are four routes over content that
fits comfortably in four sections of one document, and splitting them would buy
a router and cost a reader the ability to search the whole catalogue with
`Ctrl-F`. The page is one self-contained HTML file — inline stylesheet, inline
script, no fetched asset — so it renders identically from `file://`, from a
static host and from an archive, and a test can assert on exactly the bytes a
reader will see.

**A Rust generator, not Astro or Vite.** §6.2's deciding argument was that the
catalogue must be generated from `specs.toml` rather than hand-typed, and that
this is awkward in Hugo. It is equally awkward in Astro — the registry is TOML
read by a Rust crate that re-hashes every artefact it describes — and the house
precedent already solves it: `roteiro/website/build.sh` runs its own
repository's Rust binary and writes `website/dist`. `conform-web` does the same.
The consequence worth stating is that **no node, no npm and no bundler appear
anywhere in this repository**, and `cargo test --workspace` needs none of them.

**The "zero hand-typed URLs" claim is proved, not asserted.** Two tests, neither
of which can pass vacuously. `the_page_is_a_function_of_the_registry.rs` renders
the page from a registry of entirely fabricated values and requires that the
fabricated values appear and no real one does — anything transcribed into the
source survives that substitution and is caught.
`no_spec_facts_are_written_in_the_source.rs` scans the crate's own source for
the registry's URLs, digests, pins and names, and proves the scanner works by
planting a real URL and requiring it to be found. It found one on its first run:
a real specification name used as a sample string in a unit test. The sample was
changed rather than the rule.

---

#### The WASM/filesystem problem, resolved with evidence

§5.2 promised a WASM binding and called it "load bearing for the SPA demo".
Phase 6 then shipped a constructor that cannot work in a browser:

```c
ConformValidator* conform_validator_new(const char *spec_id,
                                        const char *registry_path);
```

A browser has no filesystem. This was flagged in Phase 6 as an open question for
Phase 7. It is answered here **by building and running the artefact**, not by
reasoning about it.

What was done: `wasm32-unknown-unknown` was installed, `conform-ffi` was built
for it as a release `cdylib` (347,412 bytes), and the module was instantiated
and called.

| Probe | Result |
|---|---|
| `cargo check -p conform-odcs --target wasm32-unknown-unknown` | **compiles**, 9.47s |
| `cargo build -p conform-ffi --target wasm32-unknown-unknown --release` | **builds**, 347 KB `.wasm` |
| module exports | all six ABI entry points, plus `memory` |
| `conform_version()` | returns `"0.1.0"` — **the C ABI works across the boundary** |
| `conform_validator_new("odcs", "specs.toml")` | returns **NULL** |
| `conform_last_error()` | `error[REG100] specs.toml: cannot read the registry: operation not supported on this platform` |
| `conform_self_test_panic()` | **traps** with `unreachable` |

Three findings, in increasing order of importance.

**1. The validator stack is wasm-compatible.** `jsonschema` 0.38, `serde_norway`
and `conform-registry` all compile to `wasm32-unknown-unknown` unmodified. The
schema validator was the plausible blocker and it is not one.

**2. The filesystem failure is real, and it is graceful.** The constructor does
not crash, corrupt memory or return a half-built handle; it returns NULL and
leaves an accurate explanation in the thread-local error slot, which is exactly
what the ABI promises for a registry that will not load. The failure mode is a
*runtime* one, though, and that is the part worth flagging: the crate **compiles
and links cleanly for wasm** and then fails on every call. Nothing in the
toolchain warns about it.

**3. `catch_unwind` does not work on this target, and the crate's own detector
says so.** `conform_self_test_panic` exists, in Phase 6's words, because "the
panic discipline has a prerequisite the linker does not check: a
`panic = "abort"` build has no unwinding to catch, and `catch_unwind` cannot
help. A binding calls it once at start-up; a process that dies there has linked
a build in which no entry point is safe to call from C."

It dies there. `wasm32-unknown-unknown` is `panic = "abort"`, the panic becomes
an `unreachable` trap, and the status code never comes back. Phase 6 built the
detector and this is the first target on which it has fired — which is the
strongest possible argument that building it was right.

**The resolution: embed the registry and the schema bytes, do not take bytes in.**

The two obvious options are a `wasm-bindgen` binding taking the schema bytes as
an argument, or embedding the vendored bytes into the artefact at build time.
**The first is wrong on the merits and must not be built.** §5.4 states what
`conform_validator_new` is for: "where the registry is read, where the digest is
verified, and where a drifted schema is refused. What comes back is a validator
that has already earned the right to issue verdicts." A bytes-in constructor
hands that decision to JavaScript. The browser would then validate against
whatever bytes the page happened to load — a schema of unverified provenance,
chosen by the caller, with the digest check reduced to decoration. That is
`odps-json-schema-latest.json` reincarnated in a different runtime, and this
project exists to prevent it.

Embedding preserves the whole chain. `include_str!` the registry, `include_bytes!`
the vendored artefacts, parse with `Registry::load_str`, and re-hash the embedded
bytes with `conform_registry::sha256_hex` before compiling the schema. Nothing is
trusted from the host, and the digest gate still runs. `OdcsValidator::from_schema_str`
is already public, so no adapter needs changing. Its honest limitation should be
recorded when it is built: because both sides are frozen into the same artefact,
the embedded check catches a registry edited without re-hashing, and cannot catch
drift on disk — that remains `conform registry verify`'s job in CI.

A third option is worth noting because it is cheaper than it looks: the existing
C ABI is *already* a linear-memory interface — pointers, lengths, out-parameters
and an explicit free — so a browser can drive it with plain
`WebAssembly.instantiate` and a `Uint8Array` view, with no `wasm-bindgen`
dependency of our own and no `wasm-pack`. That is how the probes above were run.
The one obstacle is that the module as built carries three unresolved
`__wbindgen_placeholder__` imports, reaching it transitively through
`ahash → getrandom → wasm-bindgen`; they had to be stubbed to instantiate it, and
a shipped artefact would need either the `wasm-bindgen` CLI or a build that keeps
that edge out.

**Why the demo is not in this phase.** Not for want of time. Finding 3 is a
blocker of the right kind: `conform-ffi`'s central promise is that nothing
unwinds across the boundary, and on this target that promise is provably false.
Shipping an in-browser validator whose panic net does not work — in the one crate
built around the claim that it does — would be shipping a guarantee we know to be
untrue. Resolving it means either building `std` with the exception-handling
proposal, or downgrading the guarantee for this target in writing and saying so
at the boundary. That is a `conform-ffi` decision, on `conform-ffi`'s own release
cadence, and it deserves its own phase rather than being settled as a side effect
of building a website.

So the page says so. `conform_web::site::Demo` is a typed value rather than a
paragraph, the only variant is `NotWired`, and it carries the evidence above.
Wiring the demo up means producing a different variant; there is no way to edit
the prose into claiming something untrue without the type changing under it. A
box that pretended to validate would be worse than the sentence that says it
cannot.

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
| **2** | `conform-registry` 0.1.0 + `specs.toml` for all known specs | 1 | Every vendored schema in `data-modelling-sdk` has a pinned entry; ODPS pinned at v1.0.0 (§1.2.1) and diffed against Bitol's artefact; `registry verify` green; `latest` rejected by test |
| **3** | `conform-okf` 0.1.0 — migrate Roteiro's `okf::conform` | 1, 2 | Differential parity with the pre-migration `okf::conform` over the vendored upstream corpus — see §8.1; `Finding`/`CheckReport` retired |
| **4** | `conform-cli` 0.1.0 — CLI, `--json`, then TUI | 1, 2, 3 | `--json`/non-`--json` diagnostic-set equality test; bare `validate` exits 0 with findings; TUI navigates registry and shows upstream links |
| **5** | `conform-odcs` + `conform-odps` 0.1.0 | 1, 2, §1.4 resolved | Schema-conformance fixtures pass; **oracle strategy agreed per §1.4** |
| **6** | `conform-ffi` 0.1.0 — C ABI + header (+ Python stretch) | 1 | `catch_unwind` at every boundary; valgrind/ASan clean on the C smoke test; `conform-core` tree unchanged by its existence |
| **7** | `conform-web` — SPA + WASM demo | 2, 6 | `/specs` generated from `specs.toml` with zero hand-typed URLs; demo validates in-browser |
| **8** | `conform-lexicon` | 1, 2 | Zero bespoke diagnostic/report/gate types (SC-002) |

**Critical path: 1 → 2 → 3.** Phase 3 is the highest-information phase — it is the
only one with a real regression oracle, so it is where a wrong abstraction in
Phase 1 will surface. Do not start Phase 5 before Phase 3 completes; if
`conform-core`'s shape is wrong, OKF will prove it cheaply and ODCS/ODPS will not.

### 8.1 Correction — what Phase 3's oracle actually is

Phase 3's exit criterion above used to read *"`tests/okf_interop.rs` passes
unchanged in its assertions"*, and §1.4 named that file, alongside the vendored
fixtures, as the reason OKF keeps its head start on quality of oracle. **That
was wrong about the file, and only about the file.** `okf_interop.rs` does not
exercise the conformance layer at all. Its four tests import
`rto_render::okf::read::{ReadOptions, Trust, read_bundle}` and assert over
`OkfImport` and `rto_graph::Node` — trust tiers, `meta["okf"]` payloads,
provenance. Neither `conform`, `validate_report`, `lint_report` nor
`CheckReport` appears anywhere in it. It is the **reader's** interop test, it
guards a real and valuable property, and it has nothing to say about whether
`okf::conform` was ported faithfully. Porting it here would mean porting
`okf::read` and `rto-graph` too, which is a different migration.

What Phase 3 delivered instead is a stronger oracle than the criterion asked
for, and one that does test the layer being migrated. The pre-migration
`okf::conform` was compiled standalone — verbatim apart from its two `super::`
couplings, one of which is a two-line loader — and both implementations were
run over the same bytes and diffed finding by finding: severity, rule,
document, message and order. Eight runs, and they agree on every finding in
all eight: 44 across the two upstream bundles, and 68 across two states of a
synthetic bundle built to reach the error paths the published corpus never
takes (unparseable document, circular derivation, stale index listing,
unimplemented `okf_version`, duplicate titles, malformed trust events,
incomplete attested computation). 31 of the crate's 50 codes are exercised
that way. The upstream half of those measured values is what
`upstream_bundles_are_checked_the_same_way` pins; the synthetic bundles were a
development instrument and are not committed, which is the weaker part of this
and is stated rather than glossed.

**The fixture half of §1.4's claim stands unchanged.** The vendored corpus
*is* the best oracle material in this estate, and it is now pinned in
`specs.toml` and hash-guarded in both directions. What moved is which test
reads it.

**CLI byte-identity (NFR-005) is out of scope here and remains open.** This
repository has no CLI: `conform-cli` is Phase 4. The pre-migration renderer
lives in Roteiro's `main.rs`, which also owns the escaping of foreign scalars
on the way to a terminal — `conform-okf` deliberately leaves messages raw, so
whichever renderer takes them on must escape exactly once. Phase 4 is where
that criterion can first be met, and where it should be restated.

---

## 9. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | ~~ODCS/ODPS validators do not exist~~ **RETIRED** — eighteen `validate_*_internal` fns found in `crates/core/src/validation/schema.rs`; differential testing is satisfiable | §1.2, §1.4 |
| R1a | Adapters preserve the lossy `Result<(), String>` shape instead of emitting real multi-diagnostic output | Differential test asserts new output is a strict **superset**; a fixture with ≥2 distinct faults must yield ≥2 diagnostics |
| R1b | `cfg(not(feature = "schema-validation"))` no-op stubs are copied into the new crates, reintroducing silent-pass | §5.3 rule; a compile-time test asserts no `conform-*` validator can be built into a configuration that returns success without inspecting input |
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
- **OQ-101 — Do ODCS/ODPS validators exist?** **RESOLVED: yes.** `crates/core/src/validation/schema.rs`. No longer blocking. §1.2.
- **OQ-105 — How many formats does the first wave cover?** The estate validates ~18 formats (§1.4). The four named adapter crates are wave one; confirm whether ODCL/CADS/OpenAPI/Avro/Protobuf follow as separate crates or as features of a shared adapter crate.
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

1. **§1.2 and §1.4** — the corrected evidence base. The ADR premise *holds*; the earlier "validators are missing" finding was wrong and is retracted. The real finding is that they exist but are lossy.
2. **§7.2** — per-crate versioning with `semver_check = true`, which deliberately
   inverts Roteiro's workspace default.
3. **§6.2** — the SPA stack divergence from the existing Hugo site.
4. **§5.1** — the JSON-over-FFI decision, which trades a little performance for an
   ABI that survives `Diagnostic` evolution.
