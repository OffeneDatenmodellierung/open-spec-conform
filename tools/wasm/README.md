# The WebAssembly binding

`conform-ffi`'s `wasm` feature, built and run.

```sh
tools/wasm/build.sh                                   # --target web  -> website/dist/wasm
tools/wasm/build.sh --target nodejs --out /tmp/wasm   # and the one the probe reads
node tools/wasm/probe.mjs /tmp/wasm
```

Outside the cargo workspace, like `tools/headergen` and `tools/sanitise` next
door, because these are build harnesses rather than crates and neither
`cargo test` nor `cargo deny` should have an opinion about them. The
`wasm-bindgen` **CLI** is installed here, at the version the crate's manifest
names; the `wasm-bindgen` **crate** is an optional dependency of `conform-ffi`
and reaches nobody who does not ask for it.

## Why the probe is not a `#[test]`

Three of the things worth measuring about this binding cannot be measured from
a cargo test, because a cargo test runs on the host:

| | on the host | in WebAssembly |
|---|---|---|
| a panic | caught by the test harness | traps with `unreachable` |
| `JsError` | cannot be constructed — `wasm-bindgen` panics rather than fake one | a real JavaScript `Error` |
| the filesystem | there | not there, which is the whole point |

So `crates/conform-ffi/src/wasm.rs` tests what the host can reach — that a
validator builds from the embedded bytes, that its digest gate fires, that the
contract string says what the module does — and this file tests the rest by
instantiating the real module and calling it.

## What the probe found, and what it records rather than asserts

`demonstratePanicIsFatal()` traps, and the probe requires that. What happens
*after* the trap is recorded as a note and deliberately not asserted:

```
note  after the trap, the same instance — conformVersion() returned 0.1.0
```

WebAssembly does not stop an instance that traps. The module keeps answering,
which is exactly why the panic contract calls a panic fatal anyway: the
panicking call's allocations were never released, its destructors never ran
and its invariants were never restored, and nothing in the runtime will tell a
caller that. Asserting `conformVersion()` still works would be making a promise
about a hazard; asserting it throws would be describing a runtime this is not.
Recording it keeps the measurement in the CI log where a change to it is
visible.

## The part that matters: a probe that did not run must not look clean

Every check increments a counter and the process exits non-zero if the counter
is zero — the same rule `tools/sanitise/run.sh` states for the same reason. A
checker that is not running reports no errors, and so does a library with no
errors in it; from an exit code alone the two are indistinguishable.

## Size

3,112,486 bytes, and 960,563 gzipped at `-9` — both measured, not estimated.
Most of it is `jsonschema` and the ICU tables `idna` pulls in through it, plus
102,224 bytes of embedded schema. Nothing here compresses it; a static host
serves it gzipped or brotli'd, which is where the second number comes from.
`wasm-opt` is not run — it is another tool to pin at another version, and the
win over what the linker already did with `lto = "thin"` does not pay for that
yet.
