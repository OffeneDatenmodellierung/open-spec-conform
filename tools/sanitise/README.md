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
| `leak_control.c` | never frees a library string | must be caught under valgrind; **measured, not required**, under ASan |

If the double-free control exits cleanly, `run.sh` reports that the sanitiser
is not live rather than reporting a pass. If nothing ran at all, it exits
non-zero and says so, because an exit code of 0 there would be a claim nobody
made.

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

### The C side is not instrumented on macOS, and why

Apple's clang and rustc ship different, mutually exclusive ASan runtimes: an
object compiled by Apple clang with `-fsanitize=address` references
`___asan_version_mismatch_check_apple_clang_2100`, and rustc's runtime
provides `___asan_version_mismatch_check_v8`. Linking both into one program
fails at the linker, and linking either alone leaves the other's objects
unsatisfied.

`run.sh` therefore compiles the C plainly and instruments only the Rust. What
that costs is instrumentation of loads and stores in the smoke program's own
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
this same script. That is where the valgrind arm runs, and it is where the
leak claim gets made, because valgrind's leak check needs no instrumentation
and is not optional.

## Build output

`tools/sanitise/build/` — ignored by git, like `tools/oracle/target` next
door. Delete it freely; everything in it is regenerated.
