# conform-cli

The `conform` console: the commands for navigating and verifying the
specifications this repository conforms to, and a versioned JSON report for
everything that is not a terminal.

```
conform validate <path>…         # check documents; report everything; exit 0
conform validate <path> --check  # …and gate on errors, so CI can fail
conform registry list            # the catalogue, with upstream provenance
conform registry verify          # re-hash the vendored bytes against specs.toml
```

`--json` works on any of them. `--spec <id>` restricts any of them to one
catalogued specification.

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
                └─ tui   ──► a terminal, terminal-escaped  (not yet built)
```

The engine does the work once and produces a `Run`. Every renderer reads it and
nothing else: none validates a document, none recounts a diagnostic, none
decides whether the run gates. The TUI is next, and it will be a *view* in the
strict sense rather than a second implementation of the same idea, because
there is nothing left for it to re-derive. Meanwhile,
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
a diagnostic message or into a path. On the way to a terminal, each becomes a visible `<U+XXXX>`; nothing is
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

A file is routed by its `kind` key: `DataContract` to `conform-odcs`,
`DataProduct` to `conform-odps`. A directory holding an `index.md` is read as
an OKF bundle by `conform-okf`. `--spec` overrides the routing.

Sniffing is a *routing* decision and never a verdict — the distinction matters,
because the pre-existing `validate_odcs_internal` sniffed its input, silently
validated a `dataContractSpecification` document against a different schema,
and reported the result as an ODCS verdict. Here, if the routing sends a
document to ODCS and it is not an ODCS contract, ODCS says so. A file nothing
recognises is reported under `CLI002`, never skipped, and paths that hold
nothing checkable are an error rather than a silence: a file quietly ignored is
a file everybody believes was checked.

`odcl` and `cads` are catalogued, vendored and verified with the rest, and have
no validator in this binary. `--spec odcl` refuses under `CLI005` and exits 2
rather than reporting a clean run over zero documents.

## Not yet here

The TUI (`conform tui`, and bare `conform`). Three panes — specs → documents
and their diagnostics → diagnostic detail — driven entirely from the keyboard,
with the upstream link, the pin and the drift status for the selected
specification always on screen. `ratatui` and `crossterm` are already in the
manifest and in the `cargo deny` graph; the view itself is not written. Bare
`conform` prints the help until it is.
