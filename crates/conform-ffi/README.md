# conform-ffi

A C ABI over the conformance harness: a handle, bytes in, a JSON report out,
and never an unwind across the boundary.

Optional and entirely additive. Nothing else in this repository depends on this
crate, and its absence changes nothing.

## The surface

Six functions, plus one self-test. That is the whole ABI.

```c
#include "conform.h"

const char*        conform_version(void);
ConformValidator*  conform_validator_new(const char *spec_id,
                                         const char *registry_path);
enum ConformStatus conform_validate(ConformValidator *validator,
                                    const char *document_id,
                                    const uint8_t *document, size_t document_len,
                                    char **out_json, size_t *out_len);
const char*        conform_last_error(void);
void               conform_string_free(char *text);
void               conform_validator_free(ConformValidator *validator);
enum ConformStatus conform_self_test_panic(void);
```

## The decision everything else follows from

**The boundary speaks JSON, not structs.** No Rust type crosses; a validation
returns a JSON document.

A struct-passing ABI would put `Diagnostic`'s field list into every downstream
binding's compiled artefacts, so adding one field to that type — which will
happen — would break C, Python, Node and WASM at once, and break them
*silently*: a C program compiled against the old layout links happily against
the new library and reads the wrong bytes. JSON absorbs that. The signatures
never mention a diagnostic, the shape of one is data, and a consumer that does
not know a new field ignores it.

Three version numbers, because there are three questions:

| Number | Answers | Moves when |
|---|---|---|
| `CONFORM_ABI_VERSION` (header constant) | can I link against this? | a signature or a status code changes meaning |
| `conform_version()` | which release is this? | the crate is released |
| `schema_version` (inside the JSON) | can I parse this report? | an envelope field is removed or changes meaning |

## Rules a caller has to know

- **A document full of errors is a success.** The status code says whether a
  *report was produced*; what it found is in the JSON. Reporting and gating are
  separate decisions in this family, and this boundary does not gate at all.
- **The JSON is JSON-encoded and is not escaped for a terminal.** Messages
  quote the document verbatim, so they can carry a bidirectional override or an
  ANSI sequence. Neutralising that is the job of whatever *displays* the
  report; doing it here as well would corrupt the data, because escaping is not
  idempotent. See `src/report.rs`.
- **`conform_last_error()` is per thread** and is valid only until the next
  call into the library on that thread. Copy it if you need it longer. This is
  `strerror`'s contract, deliberately.
- **`registry_path` is required.** This library does not go searching an
  embedder's filesystem for a `specs.toml`.
- **`odcs` and `odps` only.** `okf` is a *directory* of cross-referencing
  files, and a bytes-in boundary has nothing to hand it; asking for it fails
  with a message that says so.

## Panics never cross

Every entry point wraps its body in `catch_unwind`. Unwinding through a C frame
is undefined behaviour, and a library that can do it can take its host process
down over a bug in a diagnostic message.

That defence has a prerequisite the linker does not check: a `panic = "abort"`
build has no unwinding to catch. `conform_self_test_panic()` is how a binding
proves the net is real in the library it actually linked — it returns
`CONFORM_STATUS_PANIC` (and prints a Rust panic message to stderr on the way).
A process that *dies* there has linked an abort build.

**The net requires an unwinding target, and that is not hypothetical.** Phase 7
built this crate for `wasm32-unknown-unknown` and called it from Node:
`conform_version()` returned `"0.1.0"`, so the ABI works there — and
`conform_self_test_panic()` trapped with `unreachable`, because that target is
`panic = "abort"`. In such a build the pointer checks and the status codes are
still real, but a panic anywhere inside is fatal to the whole instance and a
caller has to design around it. The self-test is how you find out which world
you are in on the first call rather than on the first bug.

So the guarantee at the top of this section is stated once more, narrowed to
what is true: **nothing unwinds across the C boundary on a target that
unwinds.** The `wasm` feature below is the binding for a target that does not,
and it does not make the claim.

## The `wasm` feature

Off by default. `--features wasm` adds a `wasm-bindgen` binding — a second,
smaller surface, alongside the C ABI and changing nothing about it.

```js
import init, { ConformValidator, panicContract } from "./conform_ffi.js";
await init();
const validator = new ConformValidator("odcs");           // no path, no bytes
const report = JSON.parse(validator.validate("orders.yaml", text));
validator.free();
```

Three things are worth knowing before using it.

**A caller names a spec id, never a schema.** `conform_validator_new` takes a
registry path and a browser has no filesystem, and the fix is *not* a
constructor that takes schema bytes from the page — that hands the provenance
decision to the caller and leaves this repository's SHA-256 check as
decoration. The registry and the vendored schemas are frozen into the artefact
instead and the digest is re-checked at construction. Its honest limitation:
both sides are embedded together, so the check catches a registry edited
without re-hashing and **cannot** speak for the files on disk today. That is
still `conform registry verify`'s job.

**A panic is fatal, and there is no self-test here that says otherwise.** The
C ABI's `conform_self_test_panic` returns `CONFORM_STATUS_PANIC` to prove a
panic was caught; exporting anything with that shape from `wasm32` would be
exporting a claim, so it is absent. What is exported is `panicContract()`,
which returns the contract as a string out of the artefact itself, and
`demonstratePanicIsFatal()`, which panics and does not return. Measured, in
Node: the call throws `RuntimeError: unreachable`, **and the instance keeps
answering afterwards** — which is the hazard, not the reassurance, because the
panicking call's allocations were never released and its destructors never ran
and nothing will say so. Discard the instance.

**The feature reaches nobody who does not ask for it.**
`wasm-bindgen` is an optional dependency; `cargo tree -p conform-ffi` does not
mention it and `cargo tree -p conform-ffi --features wasm` does.
`tests/the_wasm_feature_is_additive.rs` runs in both configurations and holds
the C ABI to identical behaviour either way — including that its constructor
still refuses a registry it cannot read rather than helpfully falling back to
the embedded bytes.

```sh
tools/wasm/build.sh                                    # --target web -> website/dist/wasm
tools/wasm/build.sh --target nodejs --out /tmp/wasm
node tools/wasm/probe.mjs /tmp/wasm                    # instantiate it and call it
```

See `tools/wasm/README.md` for why the probe is a script rather than a
`#[test]`, and `src/wasm.rs` for the panic contract in full.

## The header

`include/conform.h` is generated and committed, so that a C program does not
need a Rust toolchain to find out what it is linking, and so that an ABI change
shows up in a diff.

The generator is `tools/headergen`, which is **outside the cargo workspace**:
`cbindgen` is MPL-2.0, `deny.toml` allows permissive licences only, and the
plan (§5.3, risk R4) says in as many words that `cbindgen` must not appear in
this workspace's dependency tree. `tests/the_header_matches_the_library.rs`
runs it and fails if the committed copy has drifted.

```sh
cargo run --manifest-path tools/headergen/Cargo.toml   # regenerate
```

## Memory checking

`tools/sanitise/run.sh` runs valgrind and/or AddressSanitizer over the C smoke
test, and — the part that matters — builds two programs that are *supposed* to
fail so that a clean run cannot be a checker that silently did not run. See
`tools/sanitise/README.md` for what was actually run on what.

## Building and testing

A plain `cargo test --workspace` builds this crate's library and its three
default-feature test files. It needs no C toolchain, no header generator and
no network. The two tests that need more are gated:

```sh
cargo test -p conform-ffi --features header-check   # shells out to tools/headergen
cargo test -p conform-ffi --features c-smoke        # needs a C compiler on PATH as `cc`
cargo test -p conform-ffi --features wasm           # the binding's logic, on the host
cargo test -p conform-ffi --all-features            # all three
```

`--features wasm` needs nothing extra: the module compiles for the host as
well as for `wasm32`, so the digest gate and the envelope are exercised by
`cargo test` and read by `cargo clippy` rather than left to a browser. What a
host cannot reach — the trap, and a real `JsError` — is what
`tools/wasm/probe.mjs` is for.

Both are self-sufficient from a clean checkout: neither needs a prior
`cargo build`, and neither assumes an artefact somebody else happened to leave
behind. The C smoke test asks cargo to build `libconform_ffi.a` and to say
where it put it and what else to link against — `cargo test` on its own builds
only the `rlib`, so anything that assumed otherwise passed on a warm tree and
failed on a fresh clone.

## Licence

MIT OR Apache-2.0, as the rest of the workspace.
