# conform-model-dbmv

A typed Rust model of the **Databricks Metric Views (DBMV)** document format.

```rust
use conform_model_dbmv::DBMVDocument;

let doc: DBMVDocument = serde_norway::from_str(&yaml)?;
for table in doc.source_tables() { /* … lineage */ }
```

A metric view is a semantic layer over a raw table: the dimensions you may
group by, the measures you may aggregate, the joins that widen the source, and
how the whole thing is materialised.

This crate is **types and nothing else**: three direct dependencies (`serde`,
`serde_json`, `indexmap`), and no dependency on `conform-core`, on any adapter
in this workspace, or on `jsonschema`.

## Two casings in one document, on purpose

The envelope keys are **camelCase** — `apiVersion`, `kind`, `metricViews` —
because that is the convention every other document format in this family uses.
Everything inside a metric view is **snake_case** — `display_name`,
`materialized_views` — because that is Databricks' own spelling, and rewriting
it would mean the inner content no longer matched what Databricks accepts.

The split is deliberate and load-bearing. `two_casings_in_one_document` pins
it, so a well-meaning `rename_all` on the inner types fails a test rather than
silently producing documents Databricks rejects.

## Unknown fields are kept, never dropped

Every struct ends in `#[serde(flatten)] pub extra: Extra`
(`IndexMap<String, serde_json::Value>`). Asserted by **exact equality of the
parse trees** in `tests/roundtrip.rs`.

This matters more here than anywhere else in this family. The inner content is
Databricks', it gains keys on Databricks' schedule, and — see below — there is
no schema in this repository that would notice when it has. `extra` is the only
thing standing between a read-modify-write and the silent deletion of a
`measures[].format` variant Databricks added last week.
`a_key_this_model_has_never_heard_of_survives` is the test that says so.

The one normalisation the model performs is that a collection written
explicitly empty — `dimensions: []` — is written back out absent.

## No vendored schema, and no pretending otherwise

`conform-model-odcs`, `-odps` and `-cads` were each corrected against a JSON
Schema vendored and pinned in this repository, and their fixtures come from a
corpus pinned against a real validator. **There is no vendored DBMV schema** —
`specs.toml` has no `dbmv` entry, because Databricks publishes the format as
prose rather than as a schema.

So this model is the upstream author's reading of that prose, carried across
faithfully but **not** independently verified against Databricks'
documentation, and the fixtures here are authored rather than copied. That is a
materially weaker footing than the other three model crates stand on. If DBMV
is going to be a first-class member of this family, the right next step is a
`specs.toml` entry and a vendored artefact to pin — and until there is one,
nothing here should be read as a claim that this model is correct with respect
to Databricks. It is a claim that it is correct with respect to what
`data-modelling-sdk` believed, and that it loses nothing.

## Features

| Feature | Effect |
| --- | --- |
| *(default)* | Types only. |
| `yaml` | Adds `DBMVDocument::from_yaml` / `to_yaml`, via `serde_norway`. |

The upstream model's `from_yaml`/`to_yaml` used `serde_yaml`, which is
unmaintained and pulls in `yaml-rust` (RUSTSEC-2024-0320). `cargo deny` in this
workspace rejects it, so the convenience is behind a feature and backed by
`serde_norway`, the maintained fork.

## Provenance

Ported from
[`data-modelling-sdk`](https://github.com/OffeneDatenmodellierung/data-modelling-sdk)
at commit **`22c9c218`**, from `crates/core/src/models/dbmv.rs`, under that
repository's **MIT** licence — preserved verbatim at `LICENSE-MIT-upstream`,
both at the repository root and in this crate's own directory, so that it
travels in the published `.crate` rather than only in this repository.
`data-modelling-sdk` is MIT and this workspace is
`MIT OR Apache-2.0`; MIT material redistributes under that dual offer with the
notice preserved, so there is no licence conflict to report.

Of the four Tier 1 model crates this is the closest to a straight port: DBMV
imports nothing but `serde`, it was the newest module upstream, and it needed
no key corrected.

### Changes from the upstream model

| # | Upstream | Here | Why |
| --- | --- | --- | --- |
| 1 | no `extra` on any of the nine types | `#[serde(flatten)] extra` on all nine | every unmodelled key was silently dropped — and with no schema to pin the format, that is the likeliest way for this model to do damage |
| 2 | `from_yaml`/`to_yaml` on `serde_yaml` | behind feature `yaml`, on `serde_norway` | `serde_yaml` is unmaintained and `cargo deny` rejects its `yaml-rust` dependency |
| 3 | `metric_views: Vec<_>` serialized even when empty | omitted when empty | consistency with the other three model crates; DBMV gives absent and empty the same meaning |

## Licence

`MIT OR Apache-2.0`, at your option. Upstream MIT notice preserved as above.
