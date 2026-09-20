---
Title: Create Open Spec Conform — a shared conformance/diagnostics crate for ODCS, ODPS, OKF, and future open standards
Space: ARCH
Parent: ADRs
Layout: article

# ADR-specific metadata (mark ignores unknown keys; we use them for indexing/search)
type: adr
adr-id: "0001"
status: Draft
architectural-significance: MEDIUM
domain: [data-platform, mdm-lexicon]
decision-makers: []
superseded-by:
version: "0.1"
last-modified: 2026-09-19
confluence-url:
---

# ADR-0001: Create Open Spec Conform — a shared conformance/diagnostics crate for ODCS, ODPS, OKF, and future open standards

| | |
|---|---|
| **State** | Draft |
| **Architectural Significance** | MEDIUM |
| **Domain** | data-platform, mdm-lexicon |
| **Document version** | 0.1 |

## Reference

[Link to `docs/spec/conform-core.spec.md` — the spec-kit design doc for the crate this ADR authorises. This is ADR-0001 of the Open Spec Conform project, its own repository with its own numbering sequence. It resolves FR-021 and OQ-004 of `docs/spec/mdm-lexicon-catalog.spec.md` in the MDM lexicon project, and relates to that project's own ADR-0001 (`Use OKF for narrative decision context only, not lexicon governance state`) — a same-numbered but distinct ADR in a different repository; link by full URL, not by number alone, wherever these two projects are cross-referenced.]

## Summary

*[Leave as a stub until the decision is close to Accepted.]*

## Context

Three conformance validators exist or are in flight under common ownership: ODM's ODCS validator, ODM's ODPS validator, and Roteiro's OKF validator (`rto-render`'s `okf::conform` module, built on the upstream `okf-core` model crate). A fourth — a lexicon validator for the MDM Lexicon & Managed Model Store — is now required (see `docs/spec/mdm-lexicon-catalog.spec.md`, FR-021).

Each of the three existing validators independently reinvents the same surrounding machinery: a diagnostic shape (severity, message, location), the split between reporting and gating, `--check`/`--json` flag conventions, and — most concretely — a way to resolve a cross-reference that distinguishes "the target does not exist in this document set at all" from "the target exists but was not inspected." That last distinction is not hypothetical: Roteiro's own `links --check` command conflated the two, treated a link to a non-markdown file as broken, and failed CI on 17 links that all existed on disk. The fix, once made, became the standing convention across OKF's own `links`/`trust`/`computations` commands. Building a fourth validator (lexicon) from scratch risks re-learning that exact lesson independently, and building it as a bespoke fork of any one existing validator risks coupling lexicon governance to that format's specific schema semantics.

FR-021 of the lexicon spec already commits to building the lexicon validator on top of existing tooling rather than duplicating it, and flags (OQ-004) whether ODM and Roteiro would agree to extract a shared core. Since all three existing crates and the new lexicon project are under the same ownership, that question is this ADR's to answer directly rather than a cross-org negotiation to raise elsewhere.

## Other Impacted Domains

None beyond data-platform and mdm-lexicon — this is an internal tooling decision with no direct brand-facing surface, though its output (conformance badges, CI gate behaviour) is consumed by the MDM lexicon steward experience.

## Decision makers

- [Name, Role] — primary decision maker, Data Platform (owner of all three existing crates)

## Link to initiative

[Link to `docs/spec/conform-core.spec.md`]

## Recommended option

**Option 2 — Extract a shared, format-agnostic conformance/diagnostics crate, published as Open Spec Conform (crate name `conform-core`).**

The new crate owns `Diagnostic`, `Severity`, `ConformanceReport`, a `Validator` trait, the report-vs-gate convention, and the three-way reference-resolution result (resolves / does not exist / not inspected). It owns no schema, no spec semantics, and no parsing for any individual format. ODCS-core, ODPS-core, `okf-core`, and a new lexicon-core each implement `Validator` over their own already-parsed document model and keep full ownership of their own rules. The three existing validators migrate onto it incrementally, verified by differential testing against their pre-migration output; the lexicon validator is built on it from its first commit.

This is scoped narrowly and deliberately: it extracts only the plumbing that has already been proven to repeat across formats, not an attempt to unify what ODCS, ODPS, and OKF actually validate — those remain genuinely different document models (pure schema-driven YAML/JSON versus markdown-plus-frontmatter) with no honest shared parser between them.

## Options considered + consequences

### Option 1: Merge all four into one combined spec/validator project

**Description:** A single crate owns parsing, schema definitions, and validation logic for ODCS, ODPS, OKF, and the lexicon format together.

**Consequences:**
- Pros: One project to version and release; theoretically fewer moving parts.
- Cons: ODCS/ODPS (schema-driven YAML/JSON) and OKF (markdown-plus-frontmatter) have no shared document model — combining their parsers forces either an awkward least-common-denominator format or a crate where each consumer only exercises a fraction of the code. Directly reproduces the failure mode Roteiro already hit and rejected when evaluating whether to adopt `okf-validator` wholesale: an incidental dependency needed by one format (a language-specific syntax checker, a future RDF library) becomes dead weight every other consumer has to carry, and in that case triggered thirteen `cargo deny` failures for two checks out of thirty-four. Also removes the ability for ODM and Roteiro to release ODCS/ODPS/OKF conformance work on independent timelines.
- Ops burden: High — a change to OKF-specific rules requires touching and re-releasing the same crate that ODCS depends on.
- Lock-in: High — every consumer is locked to the combined crate's release cadence and its full dependency tree.

### Option 2: Extract a shared, format-agnostic conformance/diagnostics crate (recommended)

**Description:** As described above — `conform-core` owns only the cross-cutting diagnostic/report/gate/reference-resolution machinery; each format keeps its own model, parser, and rules in its own crate depending on `conform-core`.

**Consequences:**
- Pros: Captures the machinery that has genuinely repeated three times already (soon four) without forcing unrelated document models together. Each format crate stays independently releasable and dependency-light — `conform-core` itself is targeted at zero non-`std` runtime dependencies (see the linked spec's NFR-001), so adopting it carries negligible dependency risk for any consumer. Extensible by construction: a fifth format (a new Ossie version, a future open standard) implements the existing trait surface with zero changes to `conform-core` or the other three crates. Directly resolves FR-021/OQ-004 from the lexicon spec, and gives the lexicon validator a proven pattern (report vs. gate, `--check`, the three-way reference-resolution result) to build on from day one instead of inventing its own.
- Cons: A new crate and a new repository (or workspace member) to stand up and maintain, however small. Requires migrating three existing validators' diagnostic-producing code, each needing its own differential-test verification before the legacy types are retired — real but bounded, one-time work.
- Ops burden: Lower long-term — a fix to the report/gate convention or the reference-resolution semantics (the exact class of bug that caused Roteiro's `links --check` incident) lands once and benefits every consumer, rather than needing independent fixes in three or four places.
- Lock-in: Low — the crate's scope is narrow enough that a consumer could, worst case, vendor its small trait surface rather than depend on it, though no reason to expect that's needed.

### Option 3: Do nothing — each validator keeps its own diagnostic/report logic

**Description:** ODCS-core, ODPS-core, `okf-core`, and the new lexicon-core each continue to define their own diagnostic types, report/gate conventions, and reference-resolution logic independently.

**Consequences:**
- Pros: Zero new project to stand up; no migration work on the three existing validators.
- Cons: The lexicon validator has to either duplicate the reference-resolution fix OKF already learned the hard way, or copy OKF's logic by hand with no shared ownership — the worse of the two options flagged as a risk in the original discussion. Four independent implementations of the same reporting/gating conventions means four places for that convention to drift, and no single point of truth for "what does `--check` mean" across the group's own tooling.
- Ops burden: Higher over time — proportional to the number of validators, each carrying its own copy of logic that has already needed a real bugfix once.
- Lock-in: None, but also forgoes the consistency benefit that is the actual point of having multiple validators under one ownership in the first place.

## Advice Received

| Date | Advisor | Decision version | Advice |
|------|---------|------------------|--------|
| YYYY-MM-DD | [Name, Role] | 0.1 | *[Pending first advisory pass.]* |

## Document version history

| Version | Date | Notes |
|---------|------|-------|
| 0.1 | 2026-09-19 | Initial draft. Records the decision to create Open Spec Conform (`conform-core`), resolving FR-021/OQ-004 from the MDM lexicon spec now that all four consuming crates are confirmed to be under common ownership. Numbered ADR-0001 as the first ADR of this project's own repository; note this collides in number (not content) with the MDM lexicon project's own ADR-0001 — cross-references between the two projects use full links, never bare ADR numbers. |
