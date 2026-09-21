# conform-okf

Conformance and hygiene checking for an **Open Knowledge Format** v0.2 bundle,
reported as `conform-core` diagnostics.

Two checks, deliberately two types, because they answer different questions:

| Type | Asks | Gates on |
|---|---|---|
| `OkfConformance` | is this bundle *OKF*? | errors |
| `OkfHygiene` | is this bundle *good* OKF? | nothing |

## The split is structural, not a convention

Both types implement `conform_core::Validator`, so both *report* through
`validate` — which has no way to signal failure — and both *gate* through
`check`, against a `GatePolicy` the caller names. Each states the policy it is
meant to be read under in its own `gate_policy`, and the two differ because the
questions do.

That matters because the alternative was a convention: one report type, two
functions, and a caller trusted to know which total to look at. A convention is
a thing somebody eventually gets wrong.

```rust
use conform_core::{GatePolicy, Severity, Validator};
use conform_okf::{Bundle, OkfConformance, OkfHygiene};

let bundle = Bundle::load("path/to/bundle")?;

// Reporting.
let conformance = OkfConformance.validate(&bundle);
let hygiene = OkfHygiene.validate(&bundle);

// Gating, separately, against a policy the caller names.
assert!(!hygiene.should_gate(OkfHygiene.gate_policy()));
# Ok::<(), okf_core::BundleError>(())
```

## Nothing here parses OKF

Frontmatter, trust tiers, actor classes, links, footnotes, headings,
computations and concept ids all come from [`okf-core`], an independent
pure-Rust reading of the same specification by an author who is not us. The
rules in this crate only ask questions *about* the model it returns, so when
the specification changes the parsing follows upstream and only the questions
are ours.

Re-deriving a format is how two readers of one specification end up
disagreeing. An independent implementation is also worth more than a faithful
one: a reader of our own construction, run over our own output, can only catch
a mistake we did not make twice.

Upstream's `okf-validator` does the same job and is deliberately **not** taken.
None of its dependencies is optional and it syntax-checks fenced code blocks,
so taking it means taking `rustpython-parser` — 61 crates, `LGPL-3.0-only`
through the `malachite` tree, and unmaintained advisories whose own text says
no safe upgrade exists. `cargo deny` refuses that on both counts. What is
rebuilt here is the structural half, which is the half that is about OKF; the
two code-parsing checks are absent **by default**.

## Checking code, if you want it

They are absent by default and no longer unavailable. The `syntax` feature adds
`OkfSyntax`, a third validator over the same `Bundle`, backed by
[`conform-okf-syntax`](../conform-okf-syntax) — a crate whose every code parser
is itself optional, so you choose what you are willing to compile:

| Feature | Adds | Costs |
| --- | --- | --- |
| `syntax` | `OkfSyntax`; JSON, YAML and shell quoting | one pure-Rust crate, no parser |
| `syntax-sql` | SQL, via `sqlparser` | ~17 crates, two of which compile assembly |
| `syntax-grammars` | Python, JavaScript, TypeScript, Rust, Bash | tree-sitter grammars, which compile C |

```toml
conform-okf = { version = "0.1", features = ["syntax-sql"] }
```

Two findings: `OKF310` when a `# Computation` block does not parse, which is a
warning because that is the one block something is expected to *execute*, and
`OKF007` when any other fenced block does not, which is information because the
rest of a bundle's code is illustrative and documentation is full of fragments.

With no feature named, this crate's dependency tree is `conform-core` and
`okf-core` and stops. Turning `syntax` on does not change what `OkfConformance`
or `OkfHygiene` report — the rules are a separate validator rather than a
conditional branch inside an existing one, so the same bundle produces the same
two reports in every build.

## Codes

Every finding carries a stable code. `OKF0xx` is the document, `OKF1xx` trust
and lifecycle, `OKF2xx` superseded constructs, `OKF3xx` attested computations,
`OKF4xx` references and the bundle, `OKF9xx` a bundle that could not be read at
all. `OKFLnn` is upstream hygiene rule `Ln`, and `OKFR01` is a rule of ours,
outside the `L` namespace because that namespace is upstream's.

The implementation this crate was migrated from attached a code to hygiene
findings only. `codes::HYGIENE_RULES` maps each `OKFLnn` back to the `Ln` it
carries forward, so a consumer who learned those identifiers can follow them.

## Determinism, and offline by construction

`stale_after` is checked for **syntax** and never against the clock, so a
bundle that validates today validates tomorrow. A check whose result depends on
when it ran cannot be a gate, and this one is used as one.

Nothing here performs network access. A `resource:` naming `https://…` is
reported as `Resolution::NotInspected`, never as absent — `resolve_resource`
returns `conform-core`'s three-way answer, so "the bundle does not have it" and
"nobody looked" stay different facts.

## Messages quote the bundle, and this crate does not escape them

A diagnostic's `message` interpolates values a *peer* wrote — a title, an actor
id, a path, a frontmatter scalar. Those values are not sanitised here, so a
bundle can carry a bidirectional override or a zero-width run into a message
and a renderer that writes it straight to a terminal will show a line the
bundle rewrote.

That is the renderer's boundary, not this crate's, and the split is deliberate:
escaping is not idempotent — a `\` doubles on every pass — so it has to happen
exactly once, at the point of display. The fields stay raw so a caller that
wants a path in order to *open* it still gets the path.

## What the tests are

`tests/fixtures/okf-upstream/` holds two of the four bundles published in the
specification's own repository, pinned to a commit and recorded in
`PROVENANCE.md` and in this repository's `specs.toml`. They are the closest
thing there is to an authoritative answer about what a conformant v0.2 bundle
looks like, because the people who wrote the specification wrote them.

The expected reports in `upstream_bundles_are_checked_the_same_way` are **not**
this crate's output written down after the fact. Every line was produced by
compiling the implementation this crate was migrated from, running it over the
same bytes, and diffing the two finding by finding. They are the old numbers.

[`okf-core`]: https://crates.io/crates/okf-core
