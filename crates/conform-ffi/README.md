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
A process that *dies* there has linked an abort build, and nothing in this
library is safe to call from C in that build.

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
cargo test -p conform-ffi --all-features            # both
```

## Licence

MIT OR Apache-2.0, as the rest of the workspace.
