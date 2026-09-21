# conform-cli

Check data-specification documents — Open Data Contract Standard, Open Data
Product Standard, Open Data Contract Lexicon, Open Knowledge Format — against
the real upstream schemas, and get **every** problem with a stable code, a
location and a severity, rather than the first one as a string.

```console
$ cargo install conform-cli
$ conform validate contract.yaml --registry path/to/specs.toml
```

```
conform                          # the console: specs → documents → diagnostic detail
conform tui <path>…              # …the same, loaded with documents
conform validate <path>…         # check documents; report everything; exit 0
conform validate <path> --check  # …and gate on errors, so CI can fail
conform registry list            # the catalogue, with upstream provenance
conform registry verify          # re-hash the vendored bytes against specs.toml
```

`--json` works on any of them. `--spec <id>` restricts any of them to one
catalogued specification.

## Which registry answered, and why that matters

`conform` checks documents against **vendored schemas whose provenance it
verifies first**. The catalogue that records where each schema came from — the
upstream, the immutable ref it is pinned to, and the SHA-256 of the bytes — is
`specs.toml`, and every run re-hashes the bytes against it before issuing any
verdict. A verdict against an unverified schema is not a conformance verdict.

Three places that catalogue can come from, in this order:

| Precedence | Source | `registry:` line says |
| --- | --- | --- |
| 1 | `--registry <path>` | `…/specs.toml (--registry)` |
| 2 | a `specs.toml` at or above the working directory | `…/specs.toml (found by searching upward…)` |
| 3 | the copy compiled into this binary | `embedded in conform-cli 0.1.0 (…)` |

**Every run prints which one answered.** That is not decoration. An embedded
catalogue quietly answering for a repository you believed you were checking
would be a false green of exactly the kind this project exists to prevent, so
the origin is part of the output — and in `--json`, a structured
`registry_origin` with a `kind` your pipeline can branch on.

### Installed from crates.io: it works out of the box

`cargo install conform-cli` gives you a binary that carries the catalogue and
every schema it vouches for, so it validates immediately:

```console
$ conform validate contract.yaml
conform 0.1.0 — validate
registry: embedded in conform-cli 0.1.0 (no specs.toml found on disk; pinned when this version was published)
```

The digest gate is not skipped for the embedded path — it is the same check,
through the same code, and a report says so:

```
info ODCS904 at contract.yaml
    validated against odcs@3.1.0; bytes of `schemas/odcs-json-schema-v3.1.0.json`
    embedded in conform-cli 0.1.0 at publish time and re-hashed here against the
    digest its catalogue records for upstream pin `v3.1.0`; the catalogue is
    embedded alongside them, so this says nothing about any `specs.toml` on disk
```

That last clause is the honest limit. Both sides of the comparison were frozen
into the executable at the same moment, so it proves *this binary* was built
from consistent bytes. It cannot speak for any file on disk today — checking
the working tree is `conform registry verify` inside a checkout.

### The embedded catalogue is pinned at publish time

An installed `conform 0.1.0` carries `0.1.0`'s view of upstream, and keeps it
for as long as it stays installed. If ODCS 3.2 ships next year, a
`conform 0.1.0` you installed today will still be checking against 3.1.0 and
will still say so on every report.

That is a feature where reproducibility matters and a trap where currency does.
Two ways out, and both are ordinary:

- `cargo install conform-cli --force` to take a newer release, which carries a
  newer catalogue.
- `--registry path/to/specs.toml` to check against a catalogue you control —
  a clone of
  [this repository](https://github.com/OffeneDatenmodellierung/open-spec-conform),
  or your own vendored copy. This wins over the embedded one, and the
  `registry:` line will say so.

## A worked example

```console
$ conform validate contract.yaml --registry ../open-spec-conform/specs.toml
```

```
conform 0.1.0 — validate
registry: ../open-spec-conform/specs.toml (--registry)

specs
  ✓ odcs   3.1.0    bytes matched
      pinned   v3.1.0
      upstream …the canonical URL, read from the registry rather than typed here
  …

⚠ contract.yaml  (odcs)
  warning ODCS201 at contract.yaml (/schema)
      contract catalogues no schema objects
      help: add a `schema` entry describing the dataset's shape — a contract that
            does not say what the data looks like cannot be checked against the data
      spec: odcs@3.1.0
  warning ODCS203 at contract.yaml (/team)
      contract names no owning team
      help: add a `team`, so there is somebody to ask when this contract is wrong
      spec: odcs@3.1.0

found: 0 error(s), 5 warning(s), 1 info across 1 document(s)
gate:  none — reporting only, so this run does not fail on anything it found
exit:  0
```

Five findings from one pass, each with a code you can suppress or search for,
the JSON pointer it applies to, and the clause of the standard behind it. Add
`--check` to make errors fail the run; add `--json` for the versioned envelope.
The rest of this file is why it behaves that way.

## Reporting and gating are separate decisions

`conform validate` over a document with twelve errors prints twelve findings
and **exits 0**.

That is not leniency and it is not a bug. `conform-core` models reporting and
gating as two different questions, every adapter in this family is built around
the split, and the command line is the one place a user meets it. Reporting
says what is true about a document. Gating says what should fail a build, and
it is a decision the caller makes — `--gate errors`, `--gate warnings`, or
`--check` for the common case — never one this binary makes on their behalf.

Collapsing the two is how a hygiene warning comes to fail somebody's unrelated
pull request, and a gate that cries wolf is a gate that gets switched off.

Three exit codes, deliberately distinguishable:

| Code | Means |
|---|---|
| `0` | the run happened, and the policy did not fail it |
| `1` | the run happened, and the policy failed it |
| `2` | the run could not happen — no registry, an unknown `--spec`, a bad command line |

`2` matters as much as the other two: a CI job that treats "I reached no
verdict" the same as "I reached a verdict and it was fine" will one day go
green on a tool that never ran.

## One set of facts, three renderings

```
                ┌─ human ──► stdout, terminal-escaped
 Request ─► Run ├─ json  ──► stdout, JSON-encoded, NOT terminal-escaped
                └─ tui   ──► a terminal, terminal-escaped
```

The engine does the work once, *before* the terminal is touched, and produces a
`Run`. All three renderers read it and nothing else: none validates a document,
none recounts a diagnostic, none decides whether the run gates. So the console
is a *view* in the strict sense rather than a second implementation of the same
idea — it cannot tell an operator a contract is conformant while `--json` says
it is not, and a rule added to an adapter appears in all three at once without
anybody remembering to add it to the console. And
`tests/json_changes_serialisation_only.rs` holds the human and JSON renderings
to the same diagnostics and the same exit code across the whole fixture corpus,
every gate policy and both registry commands.

## `--json` is a versioned envelope, and absence is absence

`schema_version` is `1`, and it is there before the first consumer exists
rather than after the first one breaks. Per diagnostic: code, severity,
location (document, line, column, JSON pointer), message, help and `spec_ref`.
Per specification: the upstream provenance the registry records — homepage,
repository, `pinned_ref`, steward, licence — plus the vendored path, the
digest, and what re-hashing the bytes just said.

Every provenance field the registry does not record is emitted as JSON `null`.
Never `""`, never omitted, never filled in with something plausible. Three of
the five entries in this repository's registry record no licence, and a
consumer must be able to tell that from the JSON, because "nobody wrote it
down" is the finding. `provenance_gaps` carries the registry's own list of
those absences, and a test asserts the two agree.

## Terminal escaping happens here, exactly once

Every adapter in this family quotes the document it found a fault in, and none
of them sanitises what it quotes. `conform-okf` says so in as many words: the
fields stay raw because escaping is not idempotent — a backslash doubles on
every pass — so it has to happen exactly once, at the point of display.

**This crate is that point.** A hostile document can carry `ESC [ … m`, an OSC
window-title sequence, a U+202E right-to-left override or a zero-width run into
a diagnostic message or into a path. On the way to a terminal — the human
report and the console alike — each becomes a visible `<U+XXXX>`; nothing is
deleted, because an operator shown a *different* string from the one the
document held cannot act on the finding.

`--json` is deliberately exempt. JSON string encoding is already the correct
escaping for that sink, and applying both would hand a consumer `<U+202E>`
where the document held one character. `tests/hostile_text_is_neutralised.rs`
holds both halves of that at once, with the payload travelling the ordinary
path through a real adapter rather than being injected by hand.

The human renderer emits no ANSI of its own at all — severity is a word and a
glyph, which is also what accessibility asks for and what makes `NO_COLOR`
honoured by having nothing to honour. So any escape byte on that stream could
only have come from a document, and none does.

## Which adapter checks what

Each standard is routed by the discriminator its own specification defines,
never by one borrowed from a sibling:

| Found | Routed to | Because |
|---|---|---|
| `kind: DataContract` | `conform-odcs` | Bitol's discriminator |
| `kind: DataProduct` | `conform-odps` | Bitol's discriminator |
| a root `dataContractSpecification` | `conform-lexicon` | the ODCL schema's `required` makes it mandatory |
| a directory holding an `index.md` | `conform-okf` | a bundle is a tree, not a file |

`kind` is read first, so a document that calls itself an ODCS contract goes to
ODCS whatever else it carries. `--spec` overrides the routing entirely.

Sniffing is a *routing* decision and never a verdict — the distinction matters,
because the pre-existing `validate_odcs_internal` sniffed its input, silently
validated a `dataContractSpecification` document against a different schema,
and reported the result as an ODCS verdict. Here, if the routing sends a
document to ODCS and it is not an ODCS contract, ODCS says so. Routing that
same document to `conform-lexicon` is the opposite of that defect rather than a
repetition of it: it is answered for under ODCL codes, against the ODCL schema,
and if it turns out not to be an ODCL document the ODCL adapter fails it. A
file nothing recognises is reported under `CLI002`, never skipped, and paths
that hold nothing checkable are an error rather than a silence: a file quietly
ignored is a file everybody believes was checked.

`cads` is catalogued, vendored and verified with the rest, and has no validator
in this binary. `--spec cads` refuses under `CLI005` and exits 2 rather than
reporting a clean run over zero documents.

## The console

Three panes — the catalogue, what was examined against the selected
specification, and one finding in full.

```
┌─ ▸ Specs ─────────┬─ Documents ──────────┬─ Diagnostic ──────────────────┐
│ ▸ ✓ odcs  3.1.0 ✗ │ ✗ contracts/orders…  │ ✗ error  ODCS101              │
│   ✓ odps  1.0.0 ⚠ │     ✗ error   ODCS101│                               │
│   ✓ odcl  1.2.1 · │     ⚠ warning ODCS201│ "servers" is a required       │
│   ✓ cads  1.0   · │ ✓ contracts/users.y… │ property                      │
│   ✓ okf   0.2   ⚠ │                      │                               │
├─ Upstream  [u] ───┤                      │ document   contracts/orders…  │
│ pinned   v3.1.0   │                      │ pointer    /servers           │
│ bytes    ✓ matched│                      │ spec       odcs@3.1.0         │
│ steward  Bitol    │                      │                               │
│ licence  (not rec…│                      │ ▸ bitol-io.github.io/…        │
│ ▸ bitol-io.github…│                      │                               │
└───────────────────┴──────────────────────┴───────────────────────────────┘
 [tab] pane  [↑ ↓] move  [u] upstream  [?] keys  [q] quit
```

**The upstream link is never more than two keystrokes away.** In practice it is
zero: the link, the pin and the drift status for whatever is selected are
always on screen, in the pane under the catalogue and again at the foot of the
detail pane. `u` opens the full record — every provenance field, the digest,
the fetch date, the registry's gap list, and the `notes` where the evidence for
each field and the reason for each absence is written down. The notes run to
more than a screenful, so the overlay scrolls.

**Keyboard only.** There is no mouse handling anywhere, deliberately: a
terminal tool that needs a mouse cannot be used over `ssh` on a bad link, from
a text console, or by somebody driving a screen reader. `tab` and the arrows
move between panes, `↑ ↓` (or `j k`) within one, `pgup`/`pgdn` ten rows,
`g`/`G` to the ends, `esc` closes an overlay, `q` or `ctrl-c` quits.

**No colour-only encoding.** Severity is a glyph *and* a word; drift status is
a glyph *and* a word. Colour is added on top for the people it helps and
carries nothing on its own, so the console reads the same on a monochrome
terminal or through a screen reader.

The console is tested against `ratatui`'s `TestBackend`, which renders into an
in-memory buffer — so `tests/the_console_is_a_view.rs` reads the real cells the
real widgets produced. It asserts that a hostile document reaches no pane
unescaped, that every catalogued specification is reachable and carries its
link, that navigation needs no mouse, and — in both directions — that the set
of diagnostics reachable in the console is exactly the set the run holds.

## Licence

`MIT OR Apache-2.0`, at your option, as the rest of this workspace.

One file in the published tarball is not this workspace's own work. The
embedded catalogue carries the vendored schemas with it, and one of them —
`schemas/cads.schema.json` — belongs to
[`data-modelling-sdk`](https://github.com/OffeneDatenmodellierung/data-modelling-sdk)
rather than to a standards body. That repository is **MIT**, which `specs.toml`
records along with the commit these bytes were last written at, and its notice
names a different copyright holder and year than this repository's own
`LICENSE-MIT` does. MIT's single condition is that its notice travels with the
copies, so it is preserved verbatim at `LICENSE-MIT-upstream` — here in the
crate directory, not only at the repository root, because a `.crate` archive
contains nothing from above the package root and the tarball is what a consumer
redistributes. `crates/conform-web/tests/every_crate_ships_its_licence.rs`
enforces that.
