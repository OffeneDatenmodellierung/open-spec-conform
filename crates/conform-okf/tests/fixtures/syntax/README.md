# `syntax/` — ours, and deliberately broken

Everything under `okf-upstream/` is vendored: bytes somebody else published,
pinned by digest in `SHA256SUMS` and by an entry in the repository's
`specs.toml`. Nothing here is.

`broken/` is a bundle this repository wrote, whose fenced code blocks are
wrong on purpose, and it exists because the upstream corpus cannot demonstrate
a check firing — all five of its tagged blocks parse, which is the *other*
half of what `OkfSyntax` has to be shown doing. A check nobody has watched fire
is a check nobody has tested; a check that has only ever fired is a check that
might fire on anything.

So the two fixtures are used together and answer different questions:

| Fixture | Question | Answer expected |
| --- | --- | --- |
| `okf-upstream/` | does real third-party code stay quiet? | no findings, over 5 real SQL blocks |
| `broken/` | does broken code get caught? | `OKF310` and `OKF007`, exactly where planted |

It is not vendored, has no digest, and is not covered by
`vendored_fixtures_match_the_manifest` — that test walks `okf-upstream/` and
this is a sibling of it, deliberately. Edit it freely; it is a test input, not
a record of anything.
