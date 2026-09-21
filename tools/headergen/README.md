# tools/headergen

Generates `crates/conform-ffi/include/conform.h` from that crate's source,
using the configuration in `crates/conform-ffi/cbindgen.toml`.

```sh
# from the repository root
cargo run --manifest-path tools/headergen/Cargo.toml
```

Then read the diff on `conform.h`. That diff *is* the ABI change, and it is
what a reviewer should be looking at — which is the reason the header is
committed rather than generated during the build.

## Why this is not a dependency of `conform-ffi`

`cbindgen` is MPL-2.0. `deny.toml` allows permissive licences only, and
`cargo deny --all-features` resolves the whole workspace graph including
optional features and dev-dependencies, so taking it as a dependency of
`conform-ffi` fails the licence gate:

```text
error[rejected]: failed to satisfy license requirements
   ┌─ …/cbindgen-0.29.4/Cargo.toml:41:12
41 │ license = "MPL-2.0"
   │            rejected: license is not explicitly allowed
   ├ cbindgen v0.29.4
     └── conform-ffi v0.1.0
```

That finding is correct. Adding an exception for it would blur the distinction
the gate exists to keep sharp — between what this repository *ships* and what
it *builds with* — and the plan is explicit about this particular crate (§5.3,
risk R4): `cbindgen` must not appear in the harness's tree. So the generator
lives out here, in its own cargo root, exactly as `tools/oracle` does and for
the same kind of reason.

It also means a plain `cargo test --workspace` never compiles a `syn` tree for
the benefit of one assertion in one crate.

## Who runs it

`crates/conform-ffi/tests/the_header_matches_the_library.rs`, behind
`--features header-check`. It generates into a scratch file, compares byte for
byte with the committed header, and fails with the first differing line if they
disagree. CI runs it as part of `cargo test --workspace --all-features`.

The comparison lives in the test rather than here on purpose: this program
writes a header and has no opinion about whether the result is acceptable.

## `Cargo.lock` is committed here

Unlike `tools/oracle`'s. It pins `cbindgen`, and without a pin a `cbindgen`
release that changed its output by a space would fail the drift test in a pull
request that changed no ABI at all — which is how a drift test gets disabled.
