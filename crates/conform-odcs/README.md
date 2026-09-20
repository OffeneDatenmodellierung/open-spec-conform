# conform-odcs

Conformance validation for **Open Data Contract Standard** documents that
reports every fault, not the first one.

## The problem this replaces

ODCS documents in this estate were already being validated. The function doing
it has this signature:

```rust
pub fn validate_odcs_internal(content: &str) -> Result<(), String>
```

One `String`. No severity, so "you misspelled a key" and "this is not a data
contract" arrive identically. No stable code, so suppressing a known issue
means matching on prose. No location, so nothing can be underlined in an
editor. And the JSON Schema call underneath it short-circuits, so a document
with twelve faults reports one, gets fixed, reports the next, and costs twelve
round trips.

This crate keeps that function's verdict and recovers everything it threw
away:

```
error[ODCS103] contracts/orders.yaml (/apiVersion): "v9.9.9" is not one of "v3.1.0", "v3.0.2" or 5 other candidates
error[ODCS104] contracts/orders.yaml (/id): 4815162342 is not of type "string"
error[ODCS103] contracts/orders.yaml (/kind): "DataProduct" is not one of "DataContract"
error[ODCS101] contracts/orders.yaml (/servers/0): "server" is a required property
error[ODCS101] contracts/orders.yaml (/servers/0): "type" is a required property
error[ODCS104] contracts/orders.yaml (/tags): "sales" is not of type "array"
error[ODCS102] contracts/orders.yaml: Additional properties are not allowed ('contract_status' was unexpected)
error[ODCS101] contracts/orders.yaml: "version" is a required property
warning[ODCS200] contracts/orders.yaml (/apiVersion): document declares apiVersion `v9.9.9`, and this validator carries the `v3.1.0` schema
warning[ODCS201] contracts/orders.yaml (/schema): contract catalogues no schema objects
warning[ODCS203] contracts/orders.yaml (/team): contract names no owning team
warning[ODCS204] contracts/orders.yaml (/description): contract carries no description
```

The old function's entire output for that same document is the first of those
twelve lines, and nothing else.

## The verdict is deliberately unchanged

Errors are **exactly** the published schema's findings, plus two intake
failures (the document does not parse; the document is empty). Everything this
crate adds beyond the schema is a **warning**, and the default `GatePolicy`
gates on errors alone. So this validator passes and fails the same documents
the schema does, by construction.

That is what makes adopting it safe, and it is tested rather than asserted:
`tests/oracle_agreement.rs` holds this crate's verdict against the real
`validate_odcs_internal`'s over the whole fixture corpus. See
[`tools/oracle/README.md`](../../tools/oracle/README.md) for how the oracle is
run and why its output is recorded rather than called live.

**One recorded disagreement**, and it is a finding against the old function
rather than against this crate. `validate_odcs_internal` sniffs its input: a
document carrying a `dataContractSpecification` key, or carrying `name` +
`columns` without `apiVersion`/`kind`/`schema`, is silently handed to the
**ODCL** validator and ODCL's verdict returned as though ODCS had been checked.
A well-formed ODCL document therefore comes back `Ok(())` from a function named
`validate_odcs_internal`. This crate does not sniff: asked for an ODCS verdict
it gives one.

## The codes are a public API

`ODCS0xx` intake · `ODCS1xx` schema conformance · `ODCS2xx` hygiene ·
`ODCS9xx` the validator's own setup. Errors are the `0xx`, `1xx` and `9xx`
bands; `2xx` is warnings. Full list with the meaning of each in
[`src/codes.rs`](src/codes.rs).

They are stable from 0.1.0: `tests/codes_are_a_contract.rs` spells every one
out literally, so changing a code requires changing a test that says you meant
to.

## The schema's provenance is checked, not assumed

`OdcsValidator::from_registry` resolves the schema through
[`conform-registry`](../conform-registry), which means the vendored bytes are
**re-hashed against the digest `specs.toml` records** before anything is
validated against them. A schema that has drifted is refused under `ODCS903`,
carrying the registry's own `REG022` diagnostic alongside it.

A verdict issued against a schema nobody can trace to a published standard is
not a conformance verdict. `tests/provenance_is_enforced.rs` proves the refusal
fires, and proves an honest registry still builds a validator.

## Usage

```rust
use conform_core::{GatePolicy, Severity, Validator};
use conform_odcs::{Document, OdcsValidator};

let validator = OdcsValidator::from_registry_path("specs.toml")?;
let report = validator.validate(&Document::read("contracts/orders.yaml")?);

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
- **The JSON-Schema plumbing is duplicated with `conform-odps`, deliberately.**
  Neither crate may depend on the other — they release independently against
  different upstreams — and a third shared crate was not in this task's scope.
  Worth revisiting when a third JSON-Schema-backed adapter arrives; two is not
  yet a pattern.

## Licence

Dual licensed under `MIT OR Apache-2.0`.
