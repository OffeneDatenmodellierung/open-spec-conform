# Upstream OKF bundles, vendored as conformance fixtures

## What these are

Two of the four knowledge bundles published in the **Open Knowledge Format**
specification repository, kept here so that `conform-okf` is exercised against
markdown somebody else wrote.

| Source | |
|---|---|
| Repository | <https://github.com/GoogleCloudPlatform/open-knowledge-format> |
| Path upstream | `bundles/acme_retail`, `bundles/ga4` |
| Commit | `ad30107c31c06aec8a7d5636e0d1058118604e6f` (2026-08-21) |
| Licence | Apache License 2.0 (`LICENSE.md` at the repository root; there is no `NOTICE` file) |
| Copyright | Google LLC |

**That table is second-hand and is recorded as such.** Every field in it was
transcribed from `roteiro/crates/rto-render/tests/fixtures/okf-upstream/PROVENANCE.md`,
the hand-written record that accompanied these bytes in the repository they
were taken from. Nothing here has been fetched: no network access was used at
any point, so none of it has been checked against what
`GoogleCloudPlatform/open-knowledge-format` serves today. It is accurate as to
what this estate believes and unverified as to upstream. Diffing these bytes
against the upstream tree at `ad30107c` is outstanding work; if that diff comes
back non-identical it is a finding to record here, not a reason to unpin.

## Custody chain

Copied **2026-09-20** from
`roteiro/crates/rto-render/tests/fixtures/okf-upstream/`, byte-identical
(`diff -r` clean across all twenty `.md` files). That directory was last
written there by commit `8187c90985b379cf5432ac91e39e293cd0c24a25` (2026-09-01).

`SHA256SUMS` beside this file records the SHA-256 of each of the twenty
markdown files, sorted by path, in `shasum -a 256` format. It is generated from
these bytes and nothing else, and it is what `specs.toml`'s `okf` entry pins:
OKF publishes no machine-readable schema artefact to vendor — the specification
is prose — so the vendored artefact this repository can actually hash is the
corpus itself, and a manifest is how a corpus becomes one file with one digest.

Two guards close the chain, and neither alone would:

- `conform-registry` re-hashes `SHA256SUMS` on every run, so the registry
  cannot drift from the manifest.
- `conform-okf`'s `vendored_fixtures_match_the_manifest` test re-hashes all
  twenty files and compares them to the manifest, so the manifest cannot drift
  from the fixtures. It also asserts the file count, so a fixture deleted
  outright is caught rather than silently passing.

## Why they are vendored rather than fetched

The property under test is *"`conform-okf` reads a bundle it did not write"*. A
test that fetched the bundles at run time would be a network dependency in the
test suite and would silently change meaning whenever upstream edited a file —
which is the opposite of a guard. Vendoring pins the exact bytes the assertions
were written against.

They are also the best available evidence of what the specification's own
authors consider a conformant v0.2 bundle, which is a different and stronger
thing than a fixture we wrote ourselves to match our own reading of the spec.

## Modifications

Apache-2.0 §4(b) requires that modified files carry prominent notice of the
change. Nothing inside any file has been altered — every `.md` here is
byte-identical to the copy it came from — but the **selection** was trimmed
upstream of this repository, and that record is carried forward here:

- **`acme_retail`** — every `.md` file, unmodified. Its non-markdown files were
  dropped: `viz.html` and `attesters/sql_equality.py`. Neither is read by an
  OKF consumer; the bundle walk takes `.md` only.
- **`ga4`** — trimmed to `index.md` and the `tables/` directory.
- The upstream bundles `crypto_bitcoin` and `stackoverflow` are not vendored.

No file has been reformatted, re-indented, or re-serialised. That matters more
than usual here: the point of these fixtures is their *authorial* YAML style,
and normalising them would delete the defect they catch.

## What each fixture is for

The two bundles are written in visibly different YAML styles:

- **`acme_retail`** uses **flow mappings** throughout —
  `generated: { by: …, at: … }` and `verified:\n  - { by: …, at: … }` — the
  form the specification's own examples use. It is also the only upstream
  bundle exercising `type: Attested Computation`, `stale_after`,
  `status: deprecated`, and per-source credibility signals.
- **`ga4`** is machine-serialised in PyYAML's default style: block sequences
  whose items sit at the parent key's own indentation (`tags:\n- analytics`),
  and **folded multi-line scalars** for `description`.

Between them they cover every construct a line-oriented YAML reader gets wrong.
`conform-okf` does not parse OKF itself — `okf-core` does — so what these
fixtures exercise here is the *rules*, over a model somebody else built from
somebody else's markdown.
