---
title: conform-core — a shared conformance & diagnostics harness for ODCS, ODPS, OKF, and future open standards
status: Draft
version: "0.1"
last-modified: 2026-09-19
owners: [data-platform]
related-adr: docs/adr/0001-okf-for-narrative-context-only.md
related-spec: docs/spec/mdm-lexicon-catalog.spec.md
---

# Feature Specification: `conform-core` — a shared conformance & diagnostics harness

## Overview

Four validators exist or are being built across projects under common ownership: ODM's ODCS validator, ODM's ODPS validator, Roteiro's OKF validator (`rto-render`'s `okf::conform` module, built on upstream `okf-core`), and the new lexicon validator required by the MDM Lexicon & Managed Model Store spec. Each parses a schema-defined document format and reports whether it conforms — but each currently reinvents the same surrounding machinery: how a diagnostic is shaped, how reporting differs from gating, how `--check`/`--json` behave, and how a broken cross-reference is distinguished from one that simply wasn't inspected.

This spec defines `conform-core`: a new, small, dependency-light crate that owns exactly that shared machinery and nothing about what any individual format *means*. Each format keeps its own model/parser crate — ODCS-core, ODPS-core, `okf-core`, and a new lexicon-core — and each implements a small set of `conform-core` traits over its own parsed document to get diagnostics, reporting, gating, and reference-resolution semantics for free. `conform-core` never parses ODCS, ODPS, OKF, or lexicon YAML itself, and never encodes a rule specific to any one of them.

The project exists to be extended: a fifth format (a new Ossie version, a future open standard the group adopts) should be able to depend on `conform-core` and get the same conventions without touching this crate or any of the other three.

## Goals

- One shared `Diagnostic`/`Severity`/`ConformanceReport` vocabulary used by every validator in the group, so tooling, CI output, and the eventual UI badge logic (FR-019 in the lexicon spec) can treat all four the same way.
- One shared report-vs-gate convention (`info` never fails; `validate` fails on error; `--check` gates a named condition) so a command's behaviour is predictable across formats without re-reading each crate's own rules.
- One shared reference-resolution abstraction that distinguishes "target does not exist in this document set at all" from "target exists but was not inspected" — codifying the lesson from Roteiro's own `links --check` false-positive incident, where a gate that couldn't tell the difference cried wolf and got its trust eroded.
- Zero coupling to any single format's schema, spec version, or semantics. `conform-core` compiles and is useful with no ODCS, ODPS, OKF, or lexicon code anywhere near it.
- A migration path for the three existing/in-flight validators that doesn't require a rewrite — each adopts the shared traits incrementally, keeping its own parsing and rule logic untouched.

## Non-Goals

- Parsing any of ODCS, ODPS, OKF, or the lexicon YAML. Format-specific parsing stays in each format's own `-core` crate.
- Merging the four validators into one binary or one CLI. Each keeps its own command surface; `conform-core` is a library dependency, not a replacement tool.
- Defining new validation rules for any existing format. This is an extraction of shared plumbing, not a rules rewrite — ODCS/ODPS/OKF conformance behaviour should be observably unchanged after migration.
- Taking on any dependency that only one consumer needs (a language-specific syntax checker, an RDF library, and so on). `conform-core` stays dependency-light by construction — see NFR-002.
- Cross-format semantic mapping (e.g. "this ODCS field means the same thing as this OKF concept"). That belongs to the lexicon layer, not this crate.

## User Scenarios & Testing

### Scenario 1 — New format adopts the harness

A future open standard is adopted by the group. Its owning team writes a `<format>-core` parser crate and implements `conform_core::Validator` over their parsed document type. They get `Diagnostic`, severity-based reporting, `--check`-style gating, and reference resolution without writing any of that themselves.

**Acceptance:** A minimal new validator (parser + `Validator` impl, no new `conform-core` code) produces a `ConformanceReport` usable by the same downstream tooling (CI gate, `--json` output, badge logic) as the three existing validators, with no changes to `conform-core` itself.

### Scenario 2 — ODCS/ODPS validators migrate

ODM's existing ODCS and ODPS validators are refactored to emit `conform_core::Diagnostic` instead of their current ad hoc error types, and to expose the report/gate split via `conform-core`'s conventions.

**Acceptance:** Every diagnostic the pre-migration validator produced is still produced, with equal or better location precision, verified by a differential run of the old and new validator over the same fixture set (mirroring the differential-oracle approach Roteiro already uses for OKF).

### Scenario 3 — OKF validator migrates

Roteiro's `okf::conform` module is refactored to implement `conform_core::Validator` over its existing `okf-core`-parsed model, rather than hand-rolling its own diagnostic/report types.

**Acceptance:** The existing differential test against `okf-validator` (`tests/okf_interop.rs`) continues to pass unchanged in its assertions — same diagnostic counts, same agreement — after migration; only the plumbing producing those diagnostics changes.

### Scenario 4 — Lexicon validator built directly on the harness

The new lexicon validator (required by the MDM Lexicon & Managed Model Store spec, FR-021) is written from day one against `conform-core`, with no legacy diagnostic type to migrate away from.

**Acceptance:** Lexicon validator ships with zero bespoke diagnostic/report/gate code — everything of that shape comes from `conform-core`.

### Scenario 5 — A gate that reports without failing

A CI job runs a validator's bare command (no `--check`) against a document set with real issues.

**Acceptance:** The command reports every diagnostic and exits successfully — reporting and gating are governed independently, and a command that only did one has already been identified as wrong in the OKF validator's own design history.

## Requirements

### Functional Requirements

- **FR-001**: `conform-core` MUST define a `Diagnostic` type carrying at minimum: severity, a stable machine-readable code, a human-readable message, and a location (file/document-relative, format-agnostic — e.g. a path plus optional line/field pointer).
- **FR-002**: `conform-core` MUST define severity levels sufficient to express at least "error" (gates) and "warning"/"hygiene" (reports but does not gate by default), mirroring the existing split between OKF's `validate`/error-gated checks and its `lint`/never-gated hygiene checks.
- **FR-003**: `conform-core` MUST define a `ConformanceReport` aggregating diagnostics, with helpers to filter by severity and to answer "does this report contain anything that should gate."
- **FR-004**: `conform-core` MUST define a `Validator` trait that a format's own crate implements over its own parsed document type, returning a `ConformanceReport`. The trait MUST NOT require the implementing crate to change its own parsing or in-memory model.
- **FR-005**: `conform-core` MUST provide a reference-resolution abstraction that supports at least three outcomes for a cross-reference check: resolves, does not exist in the document set at all, and exists but was not inspected — so a consuming validator can implement the "contains the target" semantics that OKF's `links`/`trust`/`computations` commands standardized on after their false-positive incident, without re-deriving it.
- **FR-006**: `conform-core` MUST codify the report-vs-gate convention as reusable helpers or a trait default: a bare validation call reports without failing; an explicit check/gate call fails on a named condition.
- **FR-007**: `conform-core` MUST codify that an output-format selector (e.g. `--json`) changes only serialization, never what is reported or whether a check gates — as a type-level or documented invariant, not left to each CLI wrapper to remember.
- **FR-008**: `conform-core` MUST NOT depend on any crate specific to a single consuming format (no YAML-frontmatter parsing, no ODCS/ODPS schema types, no markdown parsing).
- **FR-009**: Each of ODCS-core, ODPS-core, `okf-core`(Roteiro's conformance layer), and the new lexicon-core MUST implement `conform_core::Validator` and MUST retain ownership of their own schema/spec semantics — `conform-core` MUST NOT gain format-specific branches or special cases for any one of them.
- **FR-010**: Migration of the ODCS, ODPS, and OKF validators to `conform-core` MUST be verified by differential testing against each validator's pre-migration output (or, for OKF, its existing upstream differential oracle) before the legacy diagnostic types are removed.
- **FR-011**: `conform-core` MUST publish independently (its own crate, its own version, its own changelog) so a consumer can pin a version without pinning to any one format crate's release cycle.

### Non-Functional Requirements

- **NFR-001 (Dependency discipline):** `conform-core` targets zero non-`std` runtime dependencies where practical, and any dependency taken MUST be justified against the licensing/CI discipline already established elsewhere (dual `MIT OR Apache-2.0`, `cargo deny --all-features check` clean) — the same standard that led Roteiro to reject adopting `okf-validator` wholesale rather than accept a transitive Python parser for two checks out of thirty-four.
- **NFR-002 (No format leakage):** A code review of `conform-core` finding any format-specific string, schema reference, or spec-version constant is a defect, not a style note.
- **NFR-003 (Extensibility):** Adding a fifth format MUST require zero changes to `conform-core`, ODCS-core, ODPS-core, `okf-core`, or lexicon-core — only a new crate implementing the existing traits.
- **NFR-004 (House engineering conventions):** Workspace organisation, `clippy` all-deny/pedantic-warn, `deny.toml`, `rustfmt.toml`, `release-plz` release automation, dual `MIT OR Apache-2.0` licensing, and per-file coverage ratchets follow existing house conventions already applied across other Rust projects in the org.
- **NFR-005 (Backward-compatible migration):** Each of the three existing validators' observable CLI behaviour (flags, exit codes, output shape under `--json`) MUST be unchanged post-migration unless a deliberate, separately-reviewed change is made.

## Key Entities

| Entity / Crate | Owns | Depends on |
|---|---|---|
| `conform-core` | `Diagnostic`, `Severity`, `ConformanceReport`, `Validator` trait, reference-resolution abstraction, report/gate conventions | nothing format-specific |
| `odcs-core` | ODCS schema model, parser, ODCS-specific rules | `conform-core` |
| `odps-core` | ODPS schema model, parser, ODPS-specific rules | `conform-core` |
| `okf-core` (Roteiro) | OKF v0.2 model, parser, OKF-specific conformance rules (`okf::conform`) | `conform-core`, upstream `okf-core` model crate |
| `lexicon-core` (new) | Lexicon YAML schema, term URN resolution, merge-classification rules | `conform-core` |
| Format-specific CLIs (existing) | Command surface, flags, human-facing output | their own `-core` crate |

## Success Criteria

- **SC-001**: All three existing validators (ODCS, ODPS, OKF) depend on `conform-core` with zero regressions in their differential test suites.
- **SC-002**: The new lexicon validator is built with zero bespoke diagnostic/report/gate types.
- **SC-003**: `conform-core`'s own dependency tree contains no crate that exists only to serve one format's needs.
- **SC-004**: A fifth, hypothetical format can be prototyped against `conform-core` using only its published trait surface and public documentation — no need to read another format crate's source to understand the contract.

## Assumptions

- All four validators (existing and planned) remain under common ownership, so the extraction is a refactor within one organisation's control, not a cross-org negotiation (this supersedes OQ-004 from the lexicon spec — the owner has decided to proceed).
- Roteiro's existing differential-oracle test fixtures against upstream `okf-validator` remain a valid regression baseline through the migration.
- Extraction can proceed incrementally — `conform-core` ships and one validator migrates at a time, rather than requiring a synchronized cutover across all four.

## Out of Scope (this iteration)

- A shared CLI framework across the four validators' command surfaces — each keeps its own CLI; only the library-level diagnostic/report types are shared.
- Cross-format semantic linking (handled by the lexicon layer, not this crate).
- Retroactively adopting `okf-validator`'s heavier checks (code-syntax validation, etc.) — that decision stands as already recorded in Roteiro's own design history and is unaffected by this extraction.

## Open Questions / Clarifications

- **OQ-001**: Does `conform-core` live in its own repository under the same org, or as a workspace member surfaced from one of the existing repos (e.g. a new top-level crate in the Roteiro workspace, or a standalone `OffeneDatenmodellierung/conform-core`)? Recommend standalone repository, given it's a dependency of multiple otherwise-independent projects.
- **OQ-002**: Migration order — ODCS/ODPS first (simpler, no existing differential-oracle harness to preserve) or OKF first (existing test harness makes regression-proofing easier to verify)? Recommend OKF first, since its existing fixture-based differential tests give the clearest before/after regression signal.
- **OQ-003**: Should `conform-core` version 0.x track semver strictly from the first release, given at least four internal consumers will pin against it immediately?

## Review & Acceptance Checklist

- [ ] `conform-core` compiles standalone with no format-specific dependency
- [ ] `Validator` trait and `Diagnostic`/`ConformanceReport` types documented well enough for a new format to be prototyped without reading another format's source
- [ ] ODCS and ODPS validators migrated with differential-tested zero regression
- [ ] OKF validator migrated with existing `okf_interop.rs`-style differential tests passing unchanged
- [ ] Lexicon validator (from the MDM Lexicon & Managed Model Store spec) built directly on `conform-core` from first commit
- [ ] `cargo deny --all-features check` clean on `conform-core` alone
- [ ] Reference-resolution abstraction explicitly tested against the "exists but not inspected" vs "does not exist" distinction, given this is the exact bug class it exists to prevent
