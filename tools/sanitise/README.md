# Memory checking the C boundary

`run.sh` runs whatever memory checker this machine actually has over
`crates/conform-ffi/tests/smoke/conform_smoke.c`, linked against the real
library.

```sh
tools/sanitise/run.sh             # everything available
tools/sanitise/run.sh asan
tools/sanitise/run.sh valgrind
```

Outside the cargo workspace, like `tools/oracle` next door, because it is a
build harness rather than a crate and neither `cargo test` nor `cargo deny`
should have an opinion about it.

## Why this is a script and not a `#[test]`

Both checkers need the library rebuilt with different flags and relinked
against a different runtime. A `#[test]` that shelled out to `cargo` to
rebuild the crate it is part of would be a test that recompiles itself, and
there is no version of that which is not worse than a script.

## The part that matters: a clean run has to be able to be dirty

A checker that is not running reports no errors. So does a library with no
errors in it. From the exit code alone the two are indistinguishable, which
makes "ASan clean" the easiest claim in this subject area to make falsely —
forget the flag, link the wrong runtime, and a green tick arrives for nothing.

So `run.sh` builds two programs that are *supposed* to fail, and checks that
they do:

| Control | What it does | Required verdict |
|---|---|---|
| `negative_control.c` | frees a library string twice | must be caught, in both arms |
| `leak_control.c` | never frees a library string | must be caught under valgrind, and under ASan on a platform where LeakSanitizer exists; **measured, not required**, where it does not |

Two rules follow, and both were learned the hard way (see below):

1. **A control is only "caught" if the checker said so.** A non-zero exit
   status is not the test. A binary that was never built exits 127; a program
   that died for an unrelated reason exits non-zero too. The assertion is the
   checker's own report in the log, and the exit status is at most
   corroboration.
2. **A control firing is a pass, and never contributes a failure.** That is
   the whole point of a control, and it is why the verdict is counted from
   recorded facts rather than propagated from each arm's return code.

If nothing ran at all, `run.sh` exits non-zero and says so, because an exit
code of 0 there would be a claim nobody made.

## Two defects this harness had, and what they cost

Both were found by CI on Linux after the first version of this script passed
here on macOS, and both are the same shape: right on the machine they were
written on.

### The archive was on the wrong side of the source file

```
cc -I include libconform_ffi.a librustc_rt.asan.a smoke.c -o smoke   # wrong
cc -I include smoke.c libconform_ffi.a librustc_rt.asan.a -o smoke   # right
```

GNU ld walks the command line once and pulls members out of an archive only to
satisfy undefined symbols it has *already* seen, so an archive placed before
the object that needs it contributes nothing. macOS's linker resolves archives
regardless of position, so this was invisible here. On Linux every `conform_*`
symbol came back undefined and all three instrumented binaries failed to link.

### And nothing checked whether they had linked

The script then ran files that did not exist. `set -euo pipefail` gave no
protection, because bash suppresses errexit *entirely* inside a function
invoked as `f || failed=1` — which is how the arms were called. The three
failed link commands passed silently, the control "failed" with exit 127, and
the run reported a confusing `the negative control failed, but not with an
ASan report` a long way from the cause.

Every command whose status matters is now checked explicitly, a failed link is
a loud counted failure, and the `native-static-libs` list needed to link a
Rust static library is asked for rather than guessed:

```sh
cargo rustc -p conform-ffi --lib --crate-type staticlib \
    --message-format=json-render-diagnostics -- --print native-static-libs
```

which also says where the artefact is, so `CARGO_TARGET_DIR` and a different
profile both work.

## What was actually run, and where

Recorded because "valgrind/ASan clean" (plan §7.2) is two claims about two
tools, and on the machine this was written on only one of them was possible.

### `aarch64-apple-darwin` (macOS 25.6, Apple clang 21, rustc 1.98 stable + nightly)

- **valgrind: not available.** It is not packaged for Apple silicon. `run.sh`
  skips it and says so; it does not pretend.
- **AddressSanitizer: ran, clean.** Rust side built with
  `RUSTFLAGS=-Zsanitizer=address` on nightly, linked against
  `librustc-nightly_rt.asan.dylib` from the nightly sysroot.
  - The double-free control was caught — reported as a use-after-free, which
    is the right answer: `conform_string_free` reconstitutes a `CString`,
    reading the string's length before releasing it, so the second call reads
    freed memory a moment before it would have freed it twice.
  - The leak control was **not** caught. LeakSanitizer is inactive on this
    platform even with `detect_leaks=1`; a deliberate 2,819-byte leak ran to
    completion unreported. So this host verified *invalid accesses* and not
    *unreleased memory*, and `run.sh` prints that rather than rounding it up.

### `aarch64-unknown-linux-gnu` (Debian 12, gcc 12.2, valgrind 3.19.0, rustc 1.98.1 + nightly)

Run in a container on the same workstation, because CI was red and reading a
CI log is not the same as being able to change one line and look again.

- **valgrind: ran, clean.** The leak control was caught
  (`definitely lost: 2,880 bytes in 1 blocks`) and the smoke test was clean.
- **AddressSanitizer: ran, clean.** The double-free control was caught, the
  smoke test was clean, and — unlike on macOS — the leak control was caught
  too (`ERROR: LeakSanitizer: detected memory leaks`), because LeakSanitizer
  is part of ASan on Linux and on by default.

That last fact is why the ASan leak assertion is chosen by platform rather than
being a `probe` everywhere: where leak checking exists, a control that does not
fire is a broken harness and fails; where it does not exist, the same miss is a
fact about the host. Both halves were measured, not assumed.

Note the arch: this is `aarch64` Linux rather than CI's `x86_64`. The defects
found were in shell accounting and linker argument order, neither of which is
architecture-specific.

### The C side is not instrumented on macOS, and why

Apple's clang and rustc ship different, mutually exclusive ASan runtimes: an
object compiled by Apple clang with `-fsanitize=address` references
`___asan_version_mismatch_check_apple_clang_2100`, and rustc's runtime
provides `___asan_version_mismatch_check_v8`. Linking both into one program
fails at the linker, and linking either alone leaves the other's objects
unsatisfied.

`run.sh` therefore compiles the C plainly and instruments only the Rust. What
that costs is instrumentation of loads and stores in the C program's own
frames. What it keeps is the whole heap: the sanitiser's `malloc`/`free`
interceptors are process-wide once the runtime is loaded, and every allocation
this library hands a caller is on that heap — which is where an FFI ownership
bug lives. The double-free control is the proof that this configuration
detects something, and it is the reason that paragraph is a test rather than
an assurance.

On a host where clang's and rustc's LLVM versions agree, adding
`-fsanitize=address` to the C compile closes the gap. That is worth doing
where it works; it is not worth faking where it does not.

### Linux CI

`.github/workflows/ci.yml` has a `sanitise` job on `ubuntu-latest` that runs
this same script. That is where the valgrind arm runs in CI, and it is where
the leak claim gets made for every pull request.

## Build output

`tools/sanitise/build/` — ignored by git, like `tools/oracle/target` next
door. Delete it freely; everything in it is regenerated.
