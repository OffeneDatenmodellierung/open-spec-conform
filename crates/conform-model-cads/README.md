# conform-model-cads

A typed Rust model of the **Compute Asset Description Specification (CADS) v1.0**.

```rust
use conform_model_cads::CADSAsset;

let asset: CADSAsset = serde_norway::from_str(&yaml)?;
if asset.is_high_risk() { /* … */ }
for f in asset.non_compliant_frameworks() { println!("{}", f.name); }
```

CADS describes a compute asset — an AI model, an ML or data pipeline, an
application, a source or destination system — alongside the runtime it needs,
the risk it carries, the frameworks it is assessed against, and the process and
API models that describe its behaviour.

This crate is **types and nothing else**: three direct dependencies (`serde`,
`serde_json`, `indexmap`), and no dependency on `conform-core`, on any adapter
in this workspace, or on `jsonschema`.

## Closed sets, held open

Unlike ODCS and ODPS, CADS publishes real `enum`s — `kind`, `status`,
`pricing.model`, `risk.classification`, `risk.impactAreas`, the statuses under
`risk.mitigations` and `compliance.frameworks`, and the three `format` fields.
Those are Rust enums here, so the conventional values are named and matchable.

Every one of them also carries an `Other(String)` variant. A value outside the
published set is a *validation* failure and this crate is not a validator:
refusing to deserialize would leave a caller unable to inspect or repair the
very documents most in need of it, and would put a hole in the round-trip
guarantee exactly where a document is unusual. The value is kept verbatim and
written back verbatim — see `a_document_the_schema_rejects_still_round_trips`.

## Unknown fields are kept, never dropped

Every struct ends in `#[serde(flatten)] pub extra: Extra`
(`IndexMap<String, serde_json::Value>`). Asserted by **exact equality of the
parse trees** over the whole fixture corpus in `tests/roundtrip.rs`.

The one normalisation the model performs is that a collection written
explicitly empty — `tags: []` — is written back out absent. CADS gives the two
the same meaning. Tested in `an_explicitly_empty_collection_is_written_back_absent`.

The model refuses a document missing one of the six keys CADS lists as
`required`: `apiVersion`, `kind`, `id`, `name`, `version`, `status`.

## A weaker fixture provenance than the other model crates, stated plainly

`conform-model-odcs` and `conform-model-odps` copy their fixtures from an
adapter corpus that is pinned against a real upstream validator, and a test
fails if the copies drift. **CADS has no adapter in this workspace**, so there
is no such corpus. The fixtures here were authored against
`schemas/cads.schema.json` as vendored and pinned by `specs.toml`, and their
conformance was checked against that schema with `jsonschema` at the time of
writing — `conformant-full.yaml` and `conformant-minimal.yaml` validate clean;
`nonconformant-unconventional-values.yaml` fails in exactly the five places its
header says it should. That check is not automated, because automating it would
mean this crate depending on `jsonschema`, which is the one thing it must not
do. If a CADS adapter is added later, these fixtures should move to its corpus
and be copied back, as the other two crates do.

## Features

| Feature | Effect |
| --- | --- |
| *(default)* | Types only. |
| `yaml` | Adds `CADSAsset::from_yaml` / `to_yaml`, via `serde_norway`. |

## Provenance

Ported from
[`data-modelling-sdk`](https://github.com/OffeneDatenmodellierung/data-modelling-sdk)
at commit **`22c9c218`**, from `crates/core/src/models/cads.rs`, under that
repository's **MIT** licence — preserved at `LICENSE-MIT-upstream` in the
repository root. `data-modelling-sdk` is MIT and this workspace is
`MIT OR Apache-2.0`; MIT material redistributes under that dual offer with the
notice preserved, so there is no licence conflict to report.

CADS is first-party: its source of record is `data-modelling-sdk` itself, and
`specs.toml` pins the vendored schema to the commit that last wrote its bytes.
That makes this the one model crate whose upstream *specification* and upstream
*implementation* are the same repository — which is why correcting the model
against the schema mattered here too, as corrections 1 and 2 below show.

### Corrections made to the upstream model

| # | Upstream | Vendored CADS schema | Consequence upstream |
| --- | --- | --- | --- |
| 1 | `openapi_specs` under `rename_all = "camelCase"`, so the key was `openapiSpecs` | the key is **`openApiSpecs`** | every `openApiSpecs` entry dropped, and every one written was unreadable by a conforming reader |
| 2 | eleven of the struct types carried no `rename_all` at all (`CADSExternalLink`, `CADSRuntime`, `CADSRuntimeContainer`, `CADSRuntimeResources`, `CADSSLA`, `CADSSLAProperty`, `CADSTeamMember`, `CADSRiskAssessment`, `CADSRiskMitigation`, `CADSComplianceFramework`, `CADSComplianceControl`) | CADS keys are camelCase throughout | worked only because every field on those eleven happens to be a single word. Made explicit on all of them, so the next two-word field does not silently emit `snake_case` |
| 3 | closed enums with no catch-all | — | a `kind`, `status`, `format`, classification or impact area outside the set made the whole document unreadable |
| 4 | no `extra` on any of the twenty-two types | — | every unmodelled key silently dropped — the defect this crate exists to fix |
| 5 | `domain_id: Option<Uuid>` | `domainId` is **`{"type": "string"}`** | a non-UUID domain reference made the document unreadable; the model also carried a `uuid` dependency for it |
| 6 | `created_at` / `updated_at`: `Option<DateTime<Utc>>` | `createdAt` / `updatedAt` are `{"type": "string"}` | a timestamp the document wrote as `+00:00` came back as `Z`; a non-RFC3339 timestamp made the document unreadable. Kept as `String`, which removes the `chrono` dependency too |
| 7 | `unit_cost: Option<f64>` | `{"type": "number"}` | `f64` re-renders: `0.0004` is not guaranteed to come back spelled as it went in. Kept as `serde_json::Value` |
| 8 | `Option<Vec<T>>` for every collection | — | not wrong, but `Vec<T>` with `skip_serializing_if` reads better and loses nothing CADS distinguishes |

### Fields and types cut, and why

- **`tags: Vec<Tag>`** — the CADS schema publishes `tags` as
  `{"type":"array","items":{"type":"string"}}`. The upstream `Tag` parse is
  lossy (`Tag::from_str` trims and `Display` re-renders) and its hand-written
  deserializer silently discarded non-string elements. `Vec<String>` here. The
  argument is set out in `docs/findings/0002-sdk-models-coupling.md` §5.
- **`uuid` and `chrono`** — see corrections 5 and 6. Neither dependency
  survives.

## Licence

`MIT OR Apache-2.0`, at your option. Upstream MIT notice preserved as above.
