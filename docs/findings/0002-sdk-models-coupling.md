# The `data-modelling-sdk` models layer: coupling map and porting plan

**Upstream:** `data-modelling-sdk` @ `22c9c218`, `crates/core/src/models/`
**Scope:** 25 files, 13,010 lines (22,410 counting the two-path double-count in
a naive `wc -l crates/core/src/models/*.rs crates/core/src/models/**/*.rs`)
**Method:** every edge below was read out of an `use` statement. Nothing here
is inferred from a module's name.

The question this answers: *the SDK will later be refactored to depend on the
crates in this workspace rather than keep its own copies — so which of these
modules can actually be lifted, and in what order?*

---

## 1. The coupling graph

Edges are `use` statements. `serde`, `serde_json`, `chrono`, `indexmap` and
`uuid` are external and omitted except where they decide a crate boundary.

```
                       ┌──────────────── TIER 1: published standards ─────────────────┐
                       │                                                              │
  odcs/supporting.rs ──┼──► (indexmap, serde_json)          dbmv.rs ──► (serde only)  │
        ▲              │                                                              │
  odcs/property.rs ────┤                                    odps.rs ──┐               │
        ▲              │                                              │               │
  odcs/schema.rs ──────┤                                    cads.rs ──┤               │
        ▲              │                                       │      │               │
  odcs/contract.rs ────┘                                       │      │               │
                                                               ▼      ▼               │
                       └─────────────────────────────────── tag.rs ───────────────────┘
                                                               ▲   (TIER 2, leaf)
  ┌───────────────────── TIER 3: SDK domain ──────────────────┐│
  │                                                           ││
  │  enums.rs (leaf)   column.rs (leaf)                       ││
  │      ▲    ▲            ▲                                  ││
  │      │    └──────── table.rs ◄────────────────────────────┘│
  │      │                 ▲  ▲  ▲                             │
  │      ├──── relationship.rs │  │                            │
  │      │         ▲          │  │                             │
  │      ├──── domain.rs ─────┘  │   ── domain.rs also ──► cads::CADSKind (Tier 1)
  │      │         ▲             │                             │
  │      ├──── data_model.rs ────┘                             │
  │      └──── workspace.rs ──► relationship, domain_config     │
  │                                                            │
  │  decision.rs ──► tag                                       │
  │      ▲                                                     │
  │      ├── knowledge.rs ──► decision::AssetLink, tag          │
  │      └── sketch.rs ─────► decision::AssetLink, tag          │
  │                                                            │
  │  domain_config.rs (leaf)   cross_domain.rs (leaf)          │
  │  openapi.rs / bpmn.rs / dmn.rs (leaves)                    │
  └────────────────────────────────────────────────────────────┘

  odcs/converters.rs ──► odcs::{contract,property,schema,supporting}   [Tier 1]
                     ──► crate::models::column::{Column, ForeignKey, …} [Tier 3]
                     ──► crate::models::table::Table                    [Tier 3]
                     ──► crate::import::{ColumnData, TableData}         [OUTSIDE models/]
```

## 2. The specific concern, answered

> `odcs/converters.rs` is 1,551 lines and its name suggests it converts between
> ODCS types and the SDK's own `table`/`column`/`data_model` types. If so, the
> "published standards" tier is NOT cleanly separable.

**The name is accurate, and the conclusion does not follow.** `converters.rs`
is exactly that bridge, and it reaches further than feared — past `models/`
entirely, into `crate::import::{ColumnData, TableData}`
(`crates/core/src/models/odcs/converters.rs:14`).

But it is the **only** file under `models/odcs/` that reaches outside
`models/odcs/` at all. The four files that *are* the ODCS model —
`contract.rs`, `property.rs`, `schema.rs`, `supporting.rs`, 1,994 lines — import
nothing but each other, `serde`, `serde_json` and `indexmap`. Verified:

```
$ grep -n '^use ' crates/core/src/models/odcs/*.rs | grep 'crate::'
crates/core/src/models/odcs/converters.rs:14:use crate::import::{ColumnData, TableData};
crates/core/src/models/odcs/converters.rs:15:use crate::models::column::{
crates/core/src/models/odcs/converters.rs:20:use crate::models::table::Table;
```

So the coupling is **one file, not one tier**. Excluding `converters.rs` makes
the ODCS model a clean lift with a zero-`conform`, zero-SDK dependency set —
which is what `conform-model-odcs` now is.

That is not a loss for the SDK either. `converters.rs` is a *translation
between two models*, and it belongs wherever both models are in scope — which,
after the SDK adopts these crates, is the SDK. It should stay there, retargeted
at `conform-model-odcs`'s types. Lifting it into a model crate would invert the
dependency and drag the SDK's whole domain layer into a crate whose entire
selling point is that it carries none of it.

**Tier 1's only genuine need from Tier 2 is `tag.rs` (186 lines, a leaf).**
`column.rs` and `relationship.rs` were named in the brief as candidate Tier 2
members; neither is reachable from Tier 1. `column.rs` is used by `table.rs`
and by `converters.rs` (excluded). `relationship.rs` is used by `data_model.rs`
and `workspace.rs`. Both are Tier 3.

## 3. `openapi.rs`, `bpmn.rs`, `dmn.rs` are not Tier 1

The brief placed these in Tier 1 as published standards and asked where they
most honestly belong. Read, they are **not models of those standards at all.**
Each is an SDK file-registry record with the same seven fields:

```rust
pub struct OpenAPIModel {
    pub id: Uuid,            // SDK record identity
    pub domain_id: Uuid,     // SDK workspace concept
    pub name: String,
    pub file_path: String,   // "{domain_name}/{name}.openapi.yaml" — SDK on-disk layout
    pub format: OpenAPIFormat,
    pub file_size: u64,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

There is not one OpenAPI, BPMN or DMN concept in there — no paths, no
operations, no process, no decision table. `domain_id` and `file_path` are the
SDK's storage model. Publishing these as `conform-model-openapi` would tell a
crates.io reader they model OpenAPI, and they do not.

**Placement: Tier 3, with the other SDK-domain models.** 185 lines total. They
are the SDK's *index of spec files it has on disk*, which is a workspace
concept, and they should travel with `workspace.rs` and `domain.rs`.

## 4. Naming judgement for Tier 3

`conform-model-*` is the right prefix for a published specification this
project validates: it says "the model of the thing `conform-odcs` conforms
documents to". It is the wrong prefix for `sketch` (Excalidraw scenes),
`decision` (MADR records), `knowledge` (articles) or `workspace`. Nobody
conforms to those; they are one product's domain objects. Shipping
`conform-model-sketch` to crates.io would assert a standard that does not
exist.

**Proposed: `dmsdk-model-*`** — `dmsdk-model-workspace`,
`dmsdk-model-decision`, and so on. It names the thing they actually are: the
domain model of `data-modelling-sdk`. It carries no false claim of a standard,
it sorts together in a crates.io search, and when the SDK adopts them the name
already matches the consumer.

### …but Tier 3 probably should not live in this repository

Raised as an objection, not a refusal. The human has asked for it; if the
answer is still yes, `dmsdk-model-*` is the name to use.

1. **This repository's purpose is stated as conformance against published
   specifications with enforced provenance.** `specs.toml` pins every schema to
   an upstream commit; `conform-registry` enforces the pin at the moment of
   use. The Tier 3 models have no upstream specification to pin. They would be
   the first members of this workspace for which the central mechanism means
   nothing.

2. **Tier 3 is 6,155 lines and it is a cluster, not a set.** `workspace →
   relationship → table → column`, `knowledge → decision`, `sketch →
   decision`, `data_model → domain → table`. Splitting it into crates buys
   nothing — no consumer wants `decision` without `knowledge` — and one
   `dmsdk-model` crate holding all of it is a crate whose natural home is
   `data-modelling-sdk`, where it already is.

3. **Moving it does not reduce the SDK's coupling; it relocates it.** The
   benefit of Tier 1 moving here is real: the SDK stops maintaining a model of
   somebody else's standard, and a third party gets the types without the SDK.
   Neither holds for Tier 3. Nothing outside the SDK wants `ViewPosition`.

4. **`domain.rs` imports `cads::CADSKind`** — Tier 3 depends on Tier 1. That is
   the right direction and it works from either repository. It is not an
   argument for co-location.

A middle path, if the goal is for the SDK to stop carrying its own copies:
move Tier 1 here (done for ODCS), publish it, have the SDK depend on it, and
leave Tier 3 in the SDK as `data-modelling-core`'s domain layer — which is what
it is, and where its only consumer lives.

## 5. Tier 2, concretely

Only `tag.rs` is reachable from Tier 1, and it is reachable from
`odps.rs` and `cads.rs` only. Two options:

- **(a)** a `conform-model-tags` crate both depend on — 186 lines and a crate;
- **(b)** ODPS and CADS model `tags` as `Vec<String>`, which is what both
  standards publish (`ODCS Tags` is `{"type":"array","items":{"type":"string"}}`;
  the CADS schema says the same), and the `Simple`/`Pair`/`List` parse becomes
  a helper a caller applies to a string if they want it.

**(b) is correct for the model crates**, because `Tag::from_str` is lossy:
it trims and re-renders, so `SecondaryDomains:[A,  B]` round-trips as
`SecondaryDomains:[A, B]`. That is a normalisation the SDK may want and a
lossless model crate must not do silently. The parse is worth keeping — as an
opt-in helper over a `&str`, not as the storage type.

---

## 6. What was actually built, and where this stopped

### Tier 1 — done, four crates

| Crate | Upstream source | Schema corrected against | Fixture corpus |
| --- | --- | --- | --- |
| `conform-model-odcs` | `models/odcs/{contract,property,schema,supporting}.rs` | `schemas/odcs-json-schema-v3.1.0.json` | copied from `conform-odcs`, drift-tested |
| `conform-model-odps` | `models/odps.rs` | `schemas/odps-json-schema-v1.0.0.json` | copied from `conform-odps`, drift-tested |
| `conform-model-cads` | `models/cads.rs` | `schemas/cads.schema.json` | authored; conformance checked by hand |
| `conform-model-dbmv` | `models/dbmv.rs` | **none exists** | authored |

Each depends on `serde`, `serde_json` and `indexmap`, and on nothing else.
None depends on `conform-core`, on an adapter, or on `jsonschema`. No validator
was rewired: the three schema-driven adapters parse and validate exactly as
they did, and their differential oracle tests against `data-modelling-sdk` are
untouched.

Each crate's README lists its corrections and cuts in full. The pattern across
all four is worth stating once: **the upstream models disagreed with the
schemas this repository already vendors**, in ways that would have silently
dropped or mis-keyed real data. ODCS could not deserialize its own conformant
fixtures (`support` modelled as an object where the schema publishes an array;
`name` required where the schema does not require it). ODPS refused
`status: mothballed`, which is conformant. CADS wrote `openapiSpecs` where the
schema publishes `openApiSpecs`. And none of the three root types in ODCS, nor
any of the fourteen in ODPS, nor any of the twenty-two in CADS, carried a
`flatten`ed catch-all — so every key the model did not name was dropped.

That last point is the one to carry back to the SDK. `models/odcs/supporting.rs`
*did* have `extra` on eleven of its supporting types; the omission on
`ODCSContract`, `SchemaObject` and `Property` looks like an oversight rather
than a decision, and it is the difference between a round-trip and a
truncation.

### Tier 2 — resolved as unnecessary, not skipped

Only `tag.rs` was ever reachable from Tier 1, and §5 argues it should not be:
all three standards publish `tags` as an array of strings, and `Tag::from_str`
normalises. All four Tier 1 crates therefore store `Vec<String>`, and no Tier 2
crate exists because nothing needs one. `enums.rs`, `column.rs` and
`relationship.rs` were named in the brief as Tier 2 candidates; the import
graph puts all three in Tier 3.

The `Simple`/`Pair`/`List` parse is a good idea that was in the wrong place. It
is worth reviving as an opt-in helper over a `&str` — `Tag::parse(s)` returning
a view, with the `String` staying the storage type — and that is a small,
self-contained piece of work.

### Tier 3 — not ported, deliberately

Stopped here, and this is the honest boundary rather than an exhaustion point.
§3 and §4 set out the case: `openapi.rs`/`bpmn.rs`/`dmn.rs` are not models of
those standards and should not be published as though they were;
`conform-model-*` is the wrong prefix for `sketch`, `decision`, `knowledge` and
`workspace`; `dmsdk-model-*` is the right one if they move; and the argument
that they should not move at all is in §4.

That objection is raised for a decision, not asserted as one. If the answer
comes back that Tier 3 moves anyway, the work is well-defined — one
`dmsdk-model` crate, 6,155 lines, no splitting, because the cluster does not
decompose — and nothing in Tier 1 blocks it.

### Nothing left broken

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings` and `cargo deny --all-features check` are all
clean. `cargo test --workspace` went from 284 to 350 passing, `--all-features`
from 288 to 354, with nothing ignored and no assertion loosened.
`cargo tree -p conform-core` is still a single node.
