# Where these fixtures came from

Copied verbatim from `crates/conform-odps/tests/fixtures/` in this repository —
the corpus the `conform-odps` adapter is pinned against, and whose verdicts are
held to the upstream validator in `data-modelling-sdk` by that crate's
differential oracle test. Only the `conformant-*` files are copied: the
`faulty-*` ones exist to be rejected by a validator, which is not what this
crate does.

Taken at `da258cec99f08da10d5e6219d2ff3ed1e4503d7c` — the commit that last wrote those bytes, verified with
`git log -1 -- crates/conform-odps/tests/fixtures/` and contained in
`origin/main`:

    da258ce  test(conform-odcs,conform-odps): the fixture corpora, written to be argued with

## Why copies and not a path into the sibling crate

`conform-model-odps` is published to crates.io. A test reading
`../conform-odps/tests/fixtures` passes inside the workspace and fails inside
the packaged tarball, because the sibling crate is not in it. Every path this
crate touches stays inside this crate, so what `cargo test` proves here is
what a consumer gets.

The cost is that the two corpora can drift apart without a test noticing. If
the adapter corpus changes, re-copy:

```sh
cp crates/conform-odps/tests/fixtures/conformant-* \
   crates/conform-model-odps/tests/fixtures/
```
