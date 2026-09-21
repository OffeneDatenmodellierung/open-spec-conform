# conform-model-odps

A typed Rust model of the **Open Data Product Standard (ODPS) v1.0.0**.

```rust
use conform_model_odps::ODPSDataProduct;

let product: ODPSDataProduct = serde_norway::from_str(&yaml)?;
for id in product.contract_ids() { /* … resolve against conform-model-odcs */ }
```

ODPS describes a data product: the contracts it consumes on its input ports,
the contracts it publishes on its output ports, how it is managed, who owns it.
The contracts themselves are ODCS documents, modelled by
[`conform-model-odcs`](../conform-model-odcs) — which this crate does **not**
depend on, because a reader of an ODPS document does not necessarily have the
contracts it points at.

This crate is **types and nothing else**: three direct dependencies (`serde`,
`serde_json`, `indexmap`), and no dependency on `conform-core`, on any adapter
in this workspace, or on `jsonschema`. Validation of ODPS documents lives in
[`conform-odps`](../conform-odps), which is a separate decision with a separate
dependency tree.

## Unknown fields are kept, never dropped

Every struct ends in `#[serde(flatten)] pub extra: Extra`, where `Extra` is
`IndexMap<String, serde_json::Value>`. A document that goes in comes out again
with every key it arrived with, in the order it carried them. Asserted by
**exact equality of the parse trees** over the whole conformant fixture corpus
in `tests/roundtrip.rs`.

The one normalisation the model performs is that a collection written
explicitly empty — `tags: []` — is written back out absent. ODPS gives the two
the same meaning. It is stated rather than left to be discovered, and tested in
`an_explicitly_empty_collection_is_written_back_absent`.

The model refuses a document missing one of the four keys ODPS lists as
`required`: `apiVersion`, `kind`, `id`, `status`.

## Features

| Feature | Effect |
| --- | --- |
| *(default)* | Types only. |
| `yaml` | Adds `ODPSDataProduct::from_yaml` / `to_yaml`, via `serde_norway`. |

## Provenance

Ported from
[`data-modelling-sdk`](https://github.com/OffeneDatenmodellierung/data-modelling-sdk)
at commit **`22c9c218`**, from `crates/core/src/models/odps.rs`, under that
repository's **MIT** licence — preserved verbatim at `LICENSE-MIT-upstream`,
both at the repository root and in this crate's own directory, so that it
travels in the published `.crate` rather than only in this repository.
Upstream doc comments are kept wherever they carry knowledge.
`data-modelling-sdk` is MIT and this workspace is `MIT OR Apache-2.0`; MIT
material redistributes under that dual offer with the notice preserved, so
there is no licence conflict to report.

Corrected against the ODPS v1.0.0 JSON Schema vendored in this repository at
`schemas/odps-json-schema-v1.0.0.json`, whose provenance `specs.toml` pins.

### Corrections made to the upstream model

| # | Upstream | Published ODPS v1.0.0 | Consequence upstream |
| --- | --- | --- | --- |
| 1 | `status: ODPSStatus`, a closed five-variant enum | `status` is a **string**; the conventional values are published as `examples`, not as an `enum` | `conformant-unconventional-status.yaml` — a conformant document reading `status: mothballed` — fails to deserialize |
| 2 | `tags: Vec<Tag>`, parsed into a `Simple`/`Pair`/`List` enum | `Tags` is `{"type":"array","items":{"type":"string"}}` | the parse is **lossy**: `Tag::from_str` trims and `Display` re-renders, so `A:[x,&nbsp;&nbsp;y]` round-trips as `A:[x, y]`. Worse, the custom deserializer **silently discarded** any element that was not a string |
| 3 | no `extra` on any of the fourteen types | — | every unmodelled key silently dropped — the defect this crate exists to fix |
| 4 | `Option<Vec<T>>` for every collection | — | not wrong, but `Vec<T>` with `skip_serializing_if` reads better and loses nothing ODPS distinguishes |

### Fields and types cut, and why

- **`created_at` / `updated_at`: `Option<DateTime<Utc>>`** — commented upstream
  as "internal", and they are: neither `createdAt` nor `updatedAt` is an ODPS
  key. They are the SDK's storage timestamps. Cutting them removes this
  crate's only reason to depend on `chrono`. A document that carries them still
  round-trips, through `extra`.
- **`ODPSApiVersion`** — a closed enum over `v0.9.0` and `v1.0.0`. It was not
  used by `ODPSDataProduct`, whose `apiVersion` is a `String`, and a closed
  enum over version strings breaks on the next ODPS release. Cut.
- **`ODPSStatus`** — see correction 1. Cut in favour of `String`.
- **`Tag` (`models/tag.rs`)** — see correction 2. The `Simple`/`Pair`/`List`
  parse is a genuinely useful idea and the reason it is not here is only that
  it must not sit between the document and the model. It is worth reviving as
  an opt-in helper over a `&str`; `docs/findings/0002-sdk-models-coupling.md`
  §5 sets out the argument.

## Licence

`MIT OR Apache-2.0`, at your option. Upstream MIT notice preserved as above.
