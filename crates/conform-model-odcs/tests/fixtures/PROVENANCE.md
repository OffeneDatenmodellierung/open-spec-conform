# Where these fixtures came from

Copied verbatim from `crates/conform-odcs/tests/fixtures/` in this repository —
the corpus the `conform-odcs` adapter is pinned against, and whose verdicts are
held to the upstream validator in `data-modelling-sdk` by that crate's
differential oracle test. Only the `conformant-*` files are copied: the
`faulty-*` ones exist to be rejected by a validator, which is not what this
crate does.

Taken at `7766cae770763d537e368f78fe42f90bc66bee37` — the commit that last wrote those bytes, verified with
`git log -1 -- crates/conform-odcs/tests/fixtures/` and contained in
`origin/main`:

    7766cae  test(conform-odcs,conform-odps): the differential tests, and their controls

## Why copies and not a path into the sibling crate

`conform-model-odcs` is published to crates.io. A test reading
`../conform-odcs/tests/fixtures` passes inside the workspace and fails inside
the packaged tarball, because the sibling crate is not in it. Every path this
crate touches stays inside this crate, so what `cargo test` proves here is
what a consumer gets.

The cost is that the two corpora can drift apart without a test noticing. If
the adapter corpus changes, re-copy:

```sh
cp crates/conform-odcs/tests/fixtures/conformant-* \
   crates/conform-model-odcs/tests/fixtures/
```
