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

## Three defects this harness had, and what they cost

All three were found by CI after a version of this script passed on the machine
it was written on, and all three are the same shape: an assumption about the
environment that nothing asserted.

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

### And then that output was read as though it were data

The line above is the fix for the *first* two defects and the cause of the
third. `--message-format=json-render-diagnostics` keeps no structured copy of
the `native-static-libs` note: it renders it to stderr and nothing else. So the
script scraped the rendered text — and GitHub Actions sets
`CARGO_TERM_COLOR: always`, which means the line is not what it looks like:

```
ESC[1m ESC[92m note ESC[0m ESC[1m : native-static-libs: … -lm -ldl -lc ESC[0m
```

The trailing reset landed inside the last token and went to the linker:

```
/usr/bin/ld: cannot find -lc^[[0m: No such file or directory
```

The container run that had "verified" the previous fix had colour off, so the
string was clean there and the bug was invisible — the same trap as the warm
cache, one layer up.

There is a particular sting in this one. This is the crate family whose CLI has
an entire module, with three screens of justification, about neutralising ANSI
escapes so a hostile *document* cannot rewrite an operator's terminal. The
escape that actually did damage came from our own build tooling, arrived
somewhere nobody was looking, and was executed.

Three things changed, and the third is the one that matters:

1. **`--message-format=json`**, which puts that note on stdout as a structured
   record whose `message` field carries the sentence undecorated, whatever
   `CARGO_TERM_COLOR` says. The colour lives in a separate `rendered` field
   that this script never takes a value from.
2. **Colour off at the call site**, by flag and by environment. Belt and
   braces; it costs nothing and an inherited variable has now cost a red run.
3. **The answer is distrusted anyway.** Every token that will become a linker
   argument is checked against a character allow-list and *refused* if it
   fails — refused, not stripped, because an escape arriving there would mean
   the assumption about where the input comes from had quietly stopped being
   true, and repairing that silently hides the thing worth knowing. Displaying
   and executing are different obligations: `conform-cli` neutralises because
   it is about to show a human, this refuses because it is about to run a
   linker.

`check_the_parser` runs that guard against three fixtures — a clean note, one
carrying a real `ESC` byte, one carrying the `\u001b` a JSON string would have
to use — at the top of *every* invocation, as counted checks. A guard nobody
exercises is a comment. Removing the allow-list makes the script fail:

```
== the harness's own parser
   ok   — a clean note yields exactly the flags it names, in order
   FAIL — the parser accepted a library flag carrying an ANSI escape
          It would have gone to the linker. That is the defect this check exists for,
          and it is why the flags are refused rather than cleaned up.
          got: -lpthread -ldl -lc?[0m
   FAIL — the parser accepted a JSON-escaped ANSI sequence
FAILED — 2 of 8 checks across 1 checker(s).
```

Note the `?` where the escape was: a script that refuses an escape sequence and
then echoes it to a terminal has protected nothing.

The Rust half of the same lesson is in
`crates/conform-ffi/tests/support/cargo_output.rs`, held to it by
`crates/conform-ffi/tests/the_output_of_another_program_is_untrusted.rs` — which
is deliberately not behind a feature, because a test of a guard that only runs
where the guard is least needed is a test that is absent on the day it matters.

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

CI also sets `CARGO_TERM_COLOR: always`, `RUSTFLAGS: -D warnings` and
`CARGO_INCREMENTAL: 0` in the workflow's `env:` block. Reproduce with those set
— all three — or a local run is not a reproduction:

```sh
CARGO_TERM_COLOR=always RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 \
    cargo test --workspace --all-features
CARGO_TERM_COLOR=always tools/sanitise/run.sh
```

## Build output

`tools/sanitise/build/` — ignored by git, like `tools/oracle/target` next
door. Delete it freely; everything in it is regenerated.
