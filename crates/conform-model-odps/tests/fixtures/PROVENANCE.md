# Where these fixtures came from

Copied verbatim from `crates/conform-odps/tests/fixtures/` in this repository —
the corpus the `conform-odps` adapter is pinned against, and whose verdicts are
held to the upstream validator in `data-modelling-sdk` by that crate's
differential oracle test. Only the `conformant-*` files are copied: the
`faulty-*` ones exist to be rejected by a validator, which is not what this
crate does.

Taken at `f39e0d610a62b2838d307cbdae7709bd8cfa73aa` (branch `feat/typed-models`).

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
