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

---

## F-005 — The ODCL schema's entire server type-dispatch is unreachable

**Repository:** upstream `datacontract/datacontract-specification`, vendored in
this estate as `schemas/odcl-json-schema-1.2.1.json`
**Severity:** high — it is a false green, and it is 19 branches wide

`servers`' value schema is written like this:

```json
"additionalProperties": {
  "$ref": "#/$defs/BaseServer",
  "allOf": [
    { "if": { "properties": { "type": { "const": "postgres" } }, "required": ["type"] },
      "then": { "$ref": "#/$defs/PostgresServer" } },
    …18 more branches…
  ]
}
```

and the document declares `"$schema": "http://json-schema.org/draft-07/schema#"`.

Under draft-07, a `$ref` **alongside other keywords means every sibling keyword
is ignored** — the rule that changed in 2019-09, where `$ref` became an
ordinary applicator. So `BaseServer` applies, the `allOf` does not, and every
per-technology server sub-schema in the document is dead code:
`PostgresServer`, `S3Server`, `KafkaServer`, `SnowflakeServer` and fifteen
more.

`PostgresServer` requires `host`, `port`, `database` and `schema`, and types
all four. This document conforms completely:

```yaml
dataContractSpecification: 1.2.1
id: urn:datacontract:checkout:orders
info:
  title: Orders
  version: 1.0.0
servers:
  production:
    type: postgres          # required fields absent entirely
  staging:
    type: postgres
    host: 42                # and here, every one of them the wrong type
    port: "not a port"
    database: []
    schema: {}
```

`validate_odcl_internal` returns `Ok(())` for it. So does `conform-lexicon`,
deliberately — the verdict is the published schema's verdict.

**Isolated empirically**, not inferred from the specification text. A probe
schema of the same shape, varying one thing at a time
(`crates/conform-lexicon/tests/the_server_dispatch_is_dead.rs`):

| draft | `$ref` sibling present | `allOf` fires? |
|---|---|---|
| draft-07 | yes | **no** |
| draft-07 | no | yes |
| 2019-09 | yes | yes |

Two controls, one variable. The same test asserts the vendored schema still
declares draft-07, still carries the `$ref`, and still has 19 branches, so the
finding cannot quietly outlive its cause.

**Suggested fix upstream:** move the `$ref` into the `allOf` as its own
member —

```json
"allOf": [ { "$ref": "#/$defs/BaseServer" }, …the 19 branches… ]
```

— which is behaviour-preserving for `BaseServer` and revives the dispatch under
every draft. Declaring a later draft would also work, but is a larger change
with other consequences.

**What this repository does about it:** reports it and does not route around
it. Reimplementing nineteen server sub-schemas locally would make
`conform-lexicon` the only tool in the estate that rejects these documents, and
a validator nobody else agrees with is a validator nobody uses. Instead every
affected server gets an `ODCL304` note at `Severity::Info` naming the check
that did not happen — "checked against `BaseServer` only" — so the gap is
visible without the verdict moving.

---

### Orchestrator corroboration, and a one-line fix the finding under-called

Verified independently against `data-modelling-sdk/schemas/odcl-json-schema-1.2.1.json`:
`$schema` is `http://json-schema.org/draft-07/schema#`, and `properties.servers.additionalProperties`
has exactly the keys `["$ref", "allOf"]`, where `allOf` holds **19 branches, all of them `if`/`then`**.
Under draft-07 `$ref` suppresses every sibling keyword, so all 19 are dead. The finding is exact.

There is a further fact that makes the cause plainer and the repair cheaper. **The schema also uses
`$defs`, which is not a draft-07 keyword at all** — it was introduced in 2019-09, where draft-07
spells the same thing `definitions`. The file carries 22 `$defs` entries and 25 `#/$defs/`
references, and zero `#/definitions/` references. So the document is written in 2019-09 vocabulary
throughout while declaring draft-07.

The estate's own files show this is an outlier rather than a house style:

| Vendored schema | Declares | Uses |
|---|---|---|
| `odcs-json-schema-v3.1.0.json` | 2019-09 | `$defs` |
| `odps-json-schema-latest.json` | 2019-09 | `$defs` |
| `common-types-schema.json`, `dbmv`, `domain`, `system`, `workspace` | draft-07 | `definitions` |
| **`odcl-json-schema-1.2.1.json`** | **draft-07** | **`$defs`** |
| `knowledge-schema.json` | draft-07 | `$defs` |

Every other file is internally consistent. ODCL's `$schema` declaration is simply stale, and the
repair is one line — declare 2019-09, as its ODCS and ODPS siblings already do. That single change
makes `$defs` a recognised keyword *and* un-deadens all 19 branches, because 2019-09 permits `$ref`
to have siblings. This is corroborated by the adapter's own control experiment, which observed the
`allOf` firing under 2019-09 with the `$ref` left in place.

`knowledge-schema.json` has the same draft-07-plus-`$defs` inconsistency and should be checked for
the same class of silently-dead subschema.

Deciding the fix is upstream's call, not ours: bumping `$schema` will start rejecting documents that
are accepted today, which is correct but is not a patch release. Reporting it as `ODCL304` info
rather than enforcing it unilaterally remains the right call for 0.1.0.

## F-006 — `validate_odcs_internal`'s sniffing is now removable

**Repository:** `data-modelling-sdk`
**Severity:** low — this is a *resolution* note for F-001, not a new defect

F-001 records that `validate_odcs_internal` sniffs its input and hands
ODCL-shaped documents to `validate_odcl_internal`, returning that verdict as
though ODCS had been checked. The note there was that the fix requires a return
type able to distinguish "not applicable" from "valid".

That type now exists in this estate, and so does the validator it needs.
`conform-core` supplies `Severity`, a stable `DiagnosticCode` and a
`ConformanceReport` that holds many findings; `conform-lexicon` is a real ODCL
validator with the same provenance guarantees and the same diagnostic
vocabulary as `conform-odcs`. Dispatch no longer has to be hidden inside one of
the two validators to be available to a caller, because a caller can now run
either and get a report that says which standard answered.

So the sniffing can go, and the shape that replaces it is:

- `validate_odcs_internal` validates ODCS. A document that is not an ODCS
  contract fails it, with findings saying so — which is what
  `conform-lexicon`'s own corpus already demonstrates in the mirror direction:
  `faulty-odcs-document.yaml` is a perfectly good ODCS contract, and both
  `validate_odcl_internal` and `conform-lexicon` reject it as ODCL, because
  neither sniffs;
- a caller that genuinely has a mixed directory dispatches explicitly, on the
  document's own declared shape, and the dispatch is visible at the call site
  rather than buried in a function whose name says it validates one standard.
  `conform-cli`'s `discover` module already does exactly this.

This is a statement about what is now *possible*, not a claim that anybody has
done it. The change is upstream's to make, and it is a breaking one for any
caller currently relying on the sniff — which is itself worth knowing, since
that reliance is invisible in the signature today.
