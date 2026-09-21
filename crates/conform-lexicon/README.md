# conform-lexicon

Conformance validation for **Open Data Contract Lexicon** documents that
reports every fault, not the first one — and says what it did *not* check.

## A note on the name

This estate calls the standard ODCL, "Open Data Contract Language". The
document itself calls it neither: its `title` is `DataContractSpecification`
and it is published by `datacontract/datacontract-specification`. `specs.toml`
records that discrepancy in full rather than smoothing it over, and keeps
`id = "odcl"` so existing references resolve. So this crate is
`conform-lexicon`, its `SPEC_ID` is `odcl`, and its diagnostic codes are
`ODCLxxx` — a code prefix is matched on by tooling and must not drift from the
registry id it accompanies.

## The problem this replaces

ODCL documents in this estate were already being validated. The function doing
it has this signature:

```rust
pub fn validate_odcl_internal(content: &str) -> Result<(), String>
```

One `String`. No severity, so "you misspelled a key" and "this is not a data
contract" arrive identically. No stable code, so suppressing a known issue
means matching on prose. No location, so nothing can be underlined in an
editor. And the JSON Schema call underneath it short-circuits, so a document
with six faults reports one, gets fixed, reports the next, and costs six round
trips.

This crate keeps that function's verdict and recovers everything it threw
away. Over `tests/fixtures/faulty-many-faults.yaml`:

```
error[ODCL103] faulty-many-faults.yaml (/dataContractSpecification): "2.0.0" is not one of "1.2.1", "1.2.0" or 5 other candidates
error[ODCL104] faulty-many-faults.yaml (/id): 42 is not of type "string"
error[ODCL105] faulty-many-faults.yaml (/info/contact/email): "not-an-email-address" is not a "email"
error[ODCL101] faulty-many-faults.yaml (/info): "version" is a required property
error[ODCL103] faulty-many-faults.yaml (/models/order lines/fields/sku/type): "str" is not one of "number", "decimal" or 25 other candidates
error[ODCL105] faulty-many-faults.yaml (/models): "order lines" does not match "^[a-zA-Z0-9_-]+$"
warning[ODCL200] faulty-many-faults.yaml (/dataContractSpecification): document declares specification version `2.0.0`, and this validator carries the `1.2.1` schema
warning[ODCL202] faulty-many-faults.yaml (/servers): contract names no server
warning[ODCL203] faulty-many-faults.yaml (/info/owner): contract names no owner
warning[ODCL204] faulty-many-faults.yaml (/info/description): contract carries no description
info[ODCL904] faulty-many-faults.yaml: validated against odcl@1.2.1; bytes at `schemas/odcl-json-schema-1.2.1.json` verified against the digest `specs.toml` records for upstream pin `1.2.1`
```

The old function's entire output for that same document is the **first** of
those lines and nothing else — and it is the least useful one in the list.

## The verdict is deliberately unchanged

Errors are **exactly** the published schema's findings, plus two intake
failures (the document does not parse; the document is empty). Everything this
crate adds beyond the schema is a warning or an info, and the default
`GatePolicy` gates on errors alone. So this validator passes and fails the same
documents the schema does, by construction.

`tests/oracle_agreement.rs` holds that to account against the real function.
`tools/oracle` compiles `data-modelling-core` with `--features
schema-validation` and calls `validate_odcl_internal` over every fixture; what
it returned is committed verbatim, guarded by a SHA-256 per fixture and an
assertion that both sides read the same schema bytes. Over 18 fixtures
splitting 9 pass / 9 fail, **the two agree everywhere** — `KNOWN_DIVERGENCES`
is empty.

## What it adds that the schema cannot say

JSON Schema records plenty it cannot enforce, and this standard's schema
records more than most. Every rule below reads the **vendored schema** at
construction rather than a list typed into this crate, so none of them can
drift from the document it describes.

### The root object is open

Unlike ODCS, the ODCL root does not close `additionalProperties`. A misspelled
`modles:` is accepted in silence, and every rule anybody writes against
`models` simply never fires:

```
warning[ODCL206] contract.yaml (/modles): `modles` is not a key this specification defines at the root
warning[ODCL206] contract.yaml (/servicelevel): `servicelevel` is not a key this specification defines at the root
```

The published schema accepts that document, and so does this crate — the
verdict is the schema's. What changes is that the reader is told.

### Annotations no validator asserts on

Five keys carry a `deprecationMessage`, which is an annotation rather than an
assertion: every JSON Schema validator reads straight past it. `ODCL208`
reports them, quoting upstream's own wording. `info.status` publishes its
values as `examples` rather than an `enum`, so the schema cannot enforce them
either; that is `ODCL205`.

### A dispatch that never runs

`servers`' value schema is `{"$ref": "#/$defs/BaseServer", "allOf": [ …19
if/then branches… ]}`, and the schema declares draft-07 — where a `$ref`
alongside other keywords means **every sibling keyword is ignored**. So the
entire per-technology dispatch is unreachable, and a `type: postgres` server
that satisfies nothing `PostgresServer` requires conforms.

This crate does not route around it: reimplementing nineteen server
sub-schemas would make this the only tool in the estate that rejects those
documents, which is how a validator stops being used. It reports it instead,
as information, naming the check that did not happen:

```
info[ODCL304] contract.yaml (/servers/production): server `production` declares `type: postgres`, and was checked against `BaseServer` only — the `postgres` sub-schema was not applied
```

`tests/the_server_dispatch_is_dead.rs` proves the mechanism in isolation — the
same `allOf` fires with the `$ref` removed, and again with the `$ref` left in
but the draft moved to 2019-09 — and asserts the vendored schema still has that
shape, so the finding cannot outlive its cause. Recorded upstream as F-005.

## References resolve three ways, not two

A field's `references` names another model's field; a field's `$ref` names a
definition "internally or externally", in the schema's own words. The schema
types both as bare strings and says nothing about where either points. So
following one has **three** answers, and `resolve_field_reference` and
`resolve_ref` return `conform_core::Resolution` to keep them apart:

```
info[ODCL303]    (…/order_id/references): `references: orders.order_id` resolves to `/models/orders/fields/order_id` in this document
info[ODCL303]    (…/postcode/references): `references: orders.shipping.postcode` resolves to `/models/orders/fields/shipping/fields/postcode` in this document
warning[ODCL300] (…/customer_id/references): `references: customers.customer_id` names nothing in this document
warning[ODCL301] (…/discount_code/$ref): `$ref: #/definitions/absent` names nothing in this document
info[ODCL302]    (…/currency/$ref): `$ref: https://example.org/common.yaml#/definitions/currency` was not followed (outside-document-set)
info[ODCL302]    (…/sku/references): `references: orders.line_items.sku` was not followed (unsupported)
```

The last two are the point. An external `$ref` is a network question and this
crate performs no network access; `orders.line_items.sku` points into an
array's element fields, and the specification documents only `model.field` and
`model.nested_field.field` — it publishes no spelling for that, so there is no
path to follow. Reporting either as a broken reference would be a false alarm
about a target that is very probably there, and a gate that raises false alarms
gets switched off, after which it protects nothing at all.

Reporting the *resolved* ones matters too: a validator silent about a reference
it checked and a validator silent about one it never followed look identical
from outside.

## Where the schema comes from

Not from a hard-coded path. `LexiconValidator::from_registry_path` resolves the
schema through `conform-registry`, which re-hashes the vendored bytes against
the digest `specs.toml` records for them *before* anything is validated. A
schema that has drifted is refused, loudly.

That is the difference from the function this replaces, which reaches its copy
with `include_str!("../../../../schemas/odcl-json-schema-1.2.1.json")`: no
digest, no record of which upstream revision those bytes are, and nothing that
would notice an edit. The version in the path is the only provenance there is,
and a path is not evidence.

No upstream URL is written anywhere in this crate. The registry is the single
source of truth for that, and `tests/provenance_is_enforced.rs` checks the
refusal fires — each assertion with its own control, because a refusal that
fires on everything and one that fires on nothing both look like a passing test
from one side.

## This crate defines no vocabulary of its own

Every diagnostic, severity, location, report, gate and resolution here is
`conform-core`'s. There is no local `Finding`, no local `Report`, no local
`Severity`. That is what makes an ODCL failure render through the identical
path as an ODCS, ODPS or OKF failure, and
`tests/no_bespoke_vocabulary.rs` enforces it by reading this crate's own
sources.

If this crate ever *wants* a type for one of those roles, that want is a
finding about `conform-core`'s design and belongs in `docs/findings/`. It is
not something to satisfy locally.

## Messages quote the document, and this crate does not escape them

A diagnostic's `message` interpolates values the document's author wrote,
because a finding that does not quote what it found is not actionable. Those
values are **not** sanitised here. A document can carry a bidirectional
override into a message, and a renderer that writes it straight to a terminal
will show a line the document rewrote.

That is the renderer's boundary, not this crate's. Escaping is not idempotent —
a `\` doubles on every pass — so it has to happen exactly once, at the point of
display. `conform-cli` is where it happens in this repository, and
`conform-okf` documents the same convention.

## Known limitation: locations are pointers, not lines

A schema violation is located by JSON Pointer, not by line and column. The
document is parsed into a span-free value tree, so by the time the schema
objects there is no record of which line the value came from. Only a parse
failure, where the parser reports its own position, carries a line today.

Stated rather than papered over: a pointer is genuinely weaker than a line for
an editor integration. Carrying spans through the parse is the work that would
fix it, and it is not in 0.1.0.
