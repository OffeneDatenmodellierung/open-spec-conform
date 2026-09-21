# conform-model-odcs

A typed Rust model of the **Open Data Contract Standard (ODCS) v3.1.0**.

```rust
use conform_model_odcs::ODCSContract;

let contract: ODCSContract = serde_norway::from_str(&yaml)?;
let host = contract.servers[0].host.as_deref();
```

This crate is **types and nothing else**. It does not validate, does not fetch
a schema, and does not depend on `conform-core`, on any adapter in this
workspace, or on `jsonschema`. It has three direct dependencies — `serde`,
`serde_json`, `indexmap` — and fourteen crates in the whole tree, five of them
the `serde_derive` proc-macro chain that any `serde` user already has.
Validation of ODCS documents lives in
[`conform-odcs`](../conform-odcs), which is a separate decision with a separate
dependency tree, and which this crate knows nothing about.

## Unknown fields are kept, never dropped

Every struct ends in `#[serde(flatten)] pub extra: Extra`, where `Extra` is
`IndexMap<String, serde_json::Value>`. A document that goes in comes out again
with every key it arrived with: vendor extensions, keys added by a later ODCS
revision, and typos alike. `IndexMap` rather than `HashMap` so that the order
is the document's order and serialization is deterministic.

The one normalisation the model performs is that a collection written
explicitly empty — `tags: []` — is written back out absent. ODCS gives the two
the same meaning. It is stated rather than left to be discovered, and tested in
`an_explicitly_empty_collection_is_written_back_absent`.

This is the property the crate exists to provide. A model that silently dropped
what it did not recognise would turn a read-modify-write into data loss, which
is worse than having no model at all. `tests/roundtrip.rs` asserts it by **exact
equality of the two parse trees** over the whole conformant fixture corpus —
not by spot-checking fields, because a spot check is precisely the test that
passes while a vendor extension quietly disappears.

The one thing the model *does* refuse is a document missing one of the five
keys ODCS lists as `required`: `version`, `apiVersion`, `kind`, `id`, `status`.
Those five are the only non-`Option` fields of `ODCSContract`.

## Features

| Feature | Effect |
| --- | --- |
| *(default)* | Types only. |
| `yaml` | Adds `ODCSContract::from_yaml` / `to_yaml`, via `serde_norway`. |

`serde_norway` is the maintained fork of `serde_yaml` 0.9 and is what every
crate in this workspace parses YAML with. `serde_yaml` itself is unmaintained
and pulls in `yaml-rust` (RUSTSEC-2024-0320), which `cargo deny` rejects.
Enabling `yaml` does not change how any field deserializes.

## Provenance

Ported from
[`data-modelling-sdk`](https://github.com/OffeneDatenmodellierung/data-modelling-sdk)
at commit **`22c9c218`**, from `crates/core/src/models/odcs/` —
`contract.rs`, `property.rs`, `schema.rs`, `supporting.rs`, `mod.rs` — under
that repository's **MIT** licence. Doc comments are kept from the original
wherever they carry knowledge rather than restating a field name.

`data-modelling-sdk` is MIT. This workspace is `MIT OR Apache-2.0`. MIT
material may be redistributed under a dual `MIT OR Apache-2.0` offer provided
the MIT notice is preserved — which it is, here and verbatim in
`LICENSE-MIT-upstream`, kept both at the repository root and in this crate's
own directory so that it travels in the published `.crate` rather than only in
this repository — because a recipient choosing the Apache-2.0 arm still
receives the MIT grant for this portion. There is no licence conflict to
report.

The port was **corrected against the ODCS v3.1.0 JSON Schema** vendored in this
repository at `schemas/odcs-json-schema-v3.1.0.json`, whose provenance
`specs.toml` pins. The upstream model predates that vendoring and disagrees
with the published schema in several places.

### Corrections made to the upstream model

Each of these was a field the upstream model would have read into the wrong
place, or refused to read at all.

| # | Upstream | Published ODCS v3.1.0 | Consequence upstream |
| --- | --- | --- | --- |
| 1 | `support: Option<Support>` (an object) | `support` is an **array** of `SupportItem` | `conformant-full.yaml` fails to deserialize |
| 2 | `service_levels` → `serviceLevels` | the key is **`slaProperties`** | SLAs silently unreadable; `serviceLevels` is kept as a read alias, never written |
| 3 | `Price { amount, currency, billingFrequency, priceModel }` | `Pricing { priceAmount, priceCurrency, priceUnit }` | every pricing field mis-keyed |
| 4 | `name: String` (required) | `name` is **not** in `required` | `conformant-minimal.yaml` fails to deserialize |
| 5 | `status: Option<String>` | `status` **is** in `required` | a document with no status read as valid |
| 6 | `Server { server: Option, type: Option }`, no `port` | both **required**; `port` is published | `port: 5432` dropped |
| 7 | `SchemaObject` had no `logicalType` | published on `SchemaObject` | `logicalType: object` dropped |
| 8 | `Role { principal }` | `firstLevelApprovers`, `secondLevelApprovers` | approver fields dropped |
| 9 | `team: Option<Team>` | `oneOf` object *or* bare member array | the v2 array spelling fails to deserialize |
| 10 | `SchemaRelationship { from_properties: Vec, to_schema: String, to_properties: Vec }` | `from`/`to`, each `oneOf` string or array | relationships mis-keyed entirely |
| 11 | `TeamMember { name, email, role }` | `username` **required**; no `email` | member identity dropped |
| 12 | `extra` maps were `HashMap` | — | non-deterministic key order on output |
| 13 | `ODCSContract`, `SchemaObject`, `Property` had **no** `extra` at all | — | every unmodelled key silently dropped — the defect this crate exists to fix |

### Fields cut, and why

- **`links: Vec<Link>` and `terms: Option<Terms>`** — not in ODCS v3.1.0 at
  any level. An upstream invention. Cut rather than kept, so the type does not
  advertise a key the standard does not have; a document carrying them still
  round-trips, through `extra`.
- **Contract-level `quality`** — ODCS publishes `quality` on schema objects and
  on properties, not on the contract. Same treatment.
- **`Property::clustered`, `Property::default_value`, `Property::enum_values`**
  — not in `SchemaBaseProperty`. Same treatment.
- **`Property::from_flat_paths`** — ~100 lines reconstructing a property tree
  from dot-notation column paths. That is import-pipeline business logic, not a
  model operation, and it is reachable only from `odcs/converters.rs`, which is
  not ported. `flatten_to_paths`, its inverse and a plain traversal of the
  model, is kept.
- **`odcs/converters.rs` (1,551 lines) in its entirety** — it converts between
  ODCS types and `data-modelling-sdk`'s own `Table`/`Column`/`ColumnData`
  types. It is the *only* file under `models/odcs/` that reaches outside
  `models/`, and taking it would have dragged the SDK's whole domain layer in
  behind it. See the repository-level porting notes for the coupling map.
- **`uuid`** — upstream `ODCSContract::new` minted a v4 UUID for `id`. This
  crate leaves `id` empty and offers `with_id`/`new_with_id` instead, rather
  than take a random-number dependency and hand the caller an identifier they
  did not choose.

### Deliberate design decisions

- **`status` is a `String`, not an enum.** ODCS publishes the conventional
  values as `examples`, not as an `enum`, so `status: mothballed` is a
  conformant document. A closed enum here would refuse it.
- **`Property::name` and `Property::logical_type` are `Option`.** ODCS requires
  `name` of a `SchemaProperty` but not of a `SchemaItemProperty` — the
  anonymous element type under an array's `items` — and one type models both.
- **`RelationshipEnd` and `TeamRef` are untagged enums** rather than normalised
  vectors/objects, so a document written in one legal spelling is written back
  out in that spelling.

## Licence

`MIT OR Apache-2.0`, at your option. Upstream MIT notice preserved as above.
