# conform-odps

Conformance validation for **Open Data Product Standard** documents that
reports every fault, not the first one.

## The problem this replaces

ODPS documents in this estate were already being validated. The function doing
it has this signature:

```rust
pub fn validate_odps_internal(content: &str) -> Result<(), String>
```

One `String`. No severity, so "you misspelled a key" and "this is not a data
product" arrive identically. No stable code, so suppressing a known issue means
matching on prose. No location, so nothing can be underlined in an editor. And
the JSON Schema call underneath it short-circuits, so a document with twelve
faults reports one, gets fixed, reports the next, and costs twelve round trips.

There is a second, quieter defect behind the same function: it is compiled
under `#[cfg(feature = "schema-validation")]`, and the `cfg(not(...))` arm
beside it returns `Ok(())` without reading the document at all. With that
feature off, **every document passes**. Nothing in this crate is
feature-conditional, and there is no build of it in which validation is
skipped.

This crate keeps the function's verdict and recovers everything it threw away:

```
error[ODPS103] products/orders.yaml (/apiVersion): "v2.0.0" is not one of "v0.9.0" or "v1.0.0"
error[ODPS104] products/orders.yaml (/id): 4815162342 is not of type "string"
error[ODPS101] products/orders.yaml (/inputPorts/0): "version" is a required property
error[ODPS101] products/orders.yaml (/inputPorts/0): "contractId" is a required property
error[ODPS103] products/orders.yaml (/kind): "DataContract" is not one of "DataProduct"
error[ODPS102] products/orders.yaml (/outputPorts/0): Additional properties are not allowed ('refreshCadence' was unexpected)
error[ODPS104] products/orders.yaml (/tags): "sales" is not of type "array"
error[ODPS102] products/orders.yaml: Additional properties are not allowed ('product_status' was unexpected)
error[ODPS101] products/orders.yaml: "status" is a required property
warning[ODPS200] products/orders.yaml (/apiVersion): document declares apiVersion `v2.0.0`, and this validator carries the `v1.0.0` schema
warning[ODPS203] products/orders.yaml (/team): data product names no owning team
warning[ODPS204] products/orders.yaml (/description): data product carries no description
warning[ODPS206] products/orders.yaml (/version): data product records no version
```

The old function's entire output for that same document is the first of those
thirteen lines, and nothing else.

## The verdict is deliberately unchanged

Errors are **exactly** the published schema's findings, plus two intake
failures (the document does not parse; the document is empty). Everything this
crate adds beyond the schema is a **warning**, and the default `GatePolicy`
gates on errors alone. So this validator passes and fails the same documents
the schema does, by construction.

That is tested rather than asserted: `tests/oracle_agreement.rs` holds this
crate's verdict against the real `validate_odps_internal`'s over the whole
fixture corpus. See [`tools/oracle/README.md`](../../tools/oracle/README.md)
for how the oracle is run and why its output is recorded rather than called
live.

**No disagreements.** Across the whole ODPS corpus the two validators agree on
every document. The list of known divergences is empty, and the test fails if
an entry is added that does not actually diverge — so its emptiness is checked
rather than assumed.

## Hygiene rules are the schema's own prose, promoted

The ODPS schema says of `outputPorts` that "you need at least one, as a data
product without output is useless", says the same of `inputPorts`, and
describes `version` as "not required, but highly recommended" — and then
requires none of them. A sentence in a `description` cannot fail a build.
`ODPS201`, `ODPS202` and `ODPS206` are those sentences at the severity the
schema's own silence permits.

## The codes are a public API

`ODPS0xx` intake · `ODPS1xx` schema conformance · `ODPS2xx` hygiene ·
`ODPS9xx` the validator's own setup. Errors are the `0xx`, `1xx` and `9xx`
bands; `2xx` is warnings. Full list with the meaning of each in
[`src/codes.rs`](src/codes.rs).

The bands line up with `conform-odcs`'s deliberately, so a reader who has
learned one number space has learned both. The codes themselves do not:
`ODCS101` and `ODPS101` name findings against different standards and nothing
may collapse them.

They are stable from 0.1.0: `tests/codes_are_a_contract.rs` spells every one
out literally, so changing a code requires changing a test that says you meant
to.

## The schema's provenance is checked, not assumed

This is the crate that motivated the rule. The ODPS schema is vendored in
`data-modelling-sdk` as `odps-json-schema-latest.json`: no version, no source
URL, no fetch date, no `$id`. Nothing about those bytes could answer "is this
still what Bitol publishes?".

`OdpsValidator::from_registry` resolves the schema through
[`conform-registry`](../conform-registry) instead, which means the vendored
bytes are **re-hashed against the digest `specs.toml` records** before anything
is validated against them, and the entry they came from names an immutable
upstream pin (`v1.0.0`). A schema that has drifted is refused under `ODPS903`,
carrying the registry's own `REG022` diagnostic alongside it.
`tests/provenance_is_enforced.rs` proves the refusal fires, and proves an
honest registry still builds a validator.

The bytes are byte-identical to the SDK's copy — SHA-256 verified on both
sides, and the oracle recording carries both digests so the differential test
can assert the two validators read the same schema. What has changed is that
they are now pinned and checkable.

## Usage

```rust
use conform_core::{GatePolicy, Severity, Validator};
use conform_odps::{Document, OdpsValidator};

let validator = OdpsValidator::from_registry_path("specs.toml")?;
let report = validator.validate(&Document::read("products/orders.yaml")?);

for diagnostic in report.at_or_above(Severity::Warning) {
    println!("{diagnostic}");
}

std::process::exit(report.gate(GatePolicy::default()).exit_code());
```

YAML and JSON are both accepted; JSON is the subset of YAML this crate parses
without a separate code path.

## Known limitations in 0.1.0

Stated here rather than discovered later.

- **Locations are pointers, not lines.** A schema violation carries a JSON
  Pointer; only a *parse* failure carries a line and column. The document is
  parsed into a span-free value tree, so by the time the schema objects there is
  no record of which line the value came from. That is a real weakness for an
  editor integration, and carrying spans through the parse is the work that
  would fix it.
- **The schema must come from a registry.** There is no embedded copy, so a
  consumer of the published crate needs a `specs.toml` and the vendored schema
  it describes. `from_schema_str` is the escape hatch, and it makes the caller
  responsible for the provenance claim it prints.
- **The JSON-Schema plumbing is duplicated with `conform-odcs`, deliberately.**
  Neither crate may depend on the other — they release independently against
  different upstreams — and a third shared crate was not in this task's scope.
  Worth revisiting when a third JSON-Schema-backed adapter arrives; two is not
  yet a pattern.

## Licence

Dual licensed under `MIT OR Apache-2.0`.
