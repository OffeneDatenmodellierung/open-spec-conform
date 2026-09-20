# Findings for the upstream projects

Defects found in sibling repositories while building `open-spec-conform`. None
of these are bugs in this repository; each is worth raising where it lives.
Every one was verified directly against the source, not inferred.

---

## F-001 — `validate_odcs_internal` returns `Ok(())` for documents that are not ODCS

**Repository:** `data-modelling-sdk`
**File:** `crates/core/src/validation/schema.rs`, lines ~30–60
**Severity:** high — it is a false green, the worst failure mode for a validator

The function named `validate_odcs_internal` sniffs its input and, if the
document looks like ODCL, delegates to the ODCL validator and returns *that*
verdict:

```rust
/// Automatically detects and validates ODCL format files against ODCL schema
pub fn validate_odcs_internal(content: &str) -> Result<(), String> {
    …
    let is_odcl_format = if let Some(obj) = data.as_object() {
        obj.contains_key("dataContractSpecification")
            || (obj.contains_key("name")
                && obj.contains_key("columns")
                && !obj.contains_key("apiVersion")
                && !obj.contains_key("kind")
                && !obj.contains_key("schema"))
    } else { false };

    if is_odcl_format {
        return validate_odcl_internal(content);
    }
```

Consequence: a caller validating a directory of supposed ODCS contracts gets
`Ok(())` for a well-formed ODCL document. The file is *not* an ODCS contract
and was never checked as one, but nothing in the return type can say so.

Confirmed empirically by the Phase 5 oracle corpus: fixture
`sniffed-odcl-specification-key.yaml` returns `Ok(())` from the real function,
while `conform-odcs` reports 5 errors. This is the single divergence in 24
fixtures, and it is a defect in the existing function rather than in ours.

Note the doc comment describes the ODCL behaviour accurately — so the routing
is deliberate. The defect is that the *name* and the *return type* both conceal
it. A three-way outcome ("valid ODCS" / "invalid ODCS" / "not an ODCS document")
cannot be expressed by `Result<(), String>`, which is precisely the modelling
gap `conform-core`'s `Resolution` and severity-bearing diagnostics close.

**Suggested fix upstream:** either rename to reflect that it dispatches across
standards, or return a type that can distinguish "not applicable" from "valid".

---

## F-002 — `data-modelling-core` fails a security-advisory gate

**Repository:** `data-modelling-sdk`
**Severity:** medium — blocks any deny-gated workspace from depending on it

```
error[unmaintained]: yaml-rust is unmaintained.
├ ID: RUSTSEC-2024-0320
├ Solution: No safe upgrade is available!
├ yaml-rust v0.4.5
  └── data-modelling-core v2.4.0
```

`serde_yaml 0.9` is likewise unmaintained. Because `cargo deny` resolves
dev-dependencies, this prevents *any* workspace with an advisory gate from
taking `data-modelling-core` even as a dev-dependency.

This had a direct cost here: Phase 5 could not run differential tests against
the real validators as a live dev-dependency. The workaround was to move the
oracle harness to `tools/oracle/`, outside the workspace, and commit its
verbatim recorded output — guarded by per-fixture SHA-256 and a schema-digest
equality check so the recording cannot silently drift.

**Suggested fix upstream:** migrate off `yaml-rust`/`serde_yaml 0.9`. The
maintained successor is `serde_yaml_ng` or `saphyr`.

---

## F-003 — Three of four vendored schemas have no recorded licence

**Repository:** `data-modelling-sdk`
**Directory:** `schemas/`
**Severity:** medium — unknown-licence third-party bytes in the tree

No licence is recorded for the vendored ODCS, ODPS or ODCL schemas anywhere:
not in the schema files, not in `schemas/README.md`, and not in the repository
`LICENSE`, which covers that repository's own MIT-licensed code and is silent
on vendored third-party artefacts.

Recorded honestly as absent in this repository's `specs.toml` rather than
guessed. Determining the real licence requires reading the upstream Bitol
repositories.

---

## F-004 — Vendored schemas carry no `$id`, and one carried no version at all

**Repository:** `data-modelling-sdk`
**Severity:** medium — the defect that motivated `conform-registry`

Neither `odcs-json-schema-v3.1.0.json` nor `odps-json-schema-latest.json`
contains a `$id`. That is the one field which would record a schema's origin,
so the vendored bytes hold **no machine-readable link back to upstream**. With
no `PROVENANCE.md` either, the filename was the only provenance signal — and
for the ODPS file the filename said `latest`, which says nothing.

The version proved recoverable by inference: `properties.apiVersion.enum` caps
at `v1.0.0`, and a schema can only enumerate versions that existed when it was
written. The same test on the ODCS file returns `v3.1.0`, corroborating its
filename and confirming the technique. The file is therefore ODPS **v1.0.0**.

This dates the document but does **not** prove byte-identity with Bitol's
published v1.0.0 artefact — a local edit would not show up in the enum. That
diff remains outstanding.

**Suggested fix upstream:** rename to `odps-json-schema-v1.0.0.json` and adopt
a provenance record. This repository's `specs.toml` is offered as the format.
