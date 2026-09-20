#!/usr/bin/env bash
#
# Run whatever memory checker this machine actually has over the C smoke test.
#
# Plan §7.2 asks for "valgrind/ASan clean on the C smoke test" as Phase 6's
# exit criterion. Those are two different tools with two different
# availabilities, and on a modern macOS one of them does not exist at all. This
# script determines what is here, runs it, and — this is the part that matters
# — refuses to report success for a checker it did not manage to run.
#
# Usage:
#   tools/sanitise/run.sh            # run everything available
#   tools/sanitise/run.sh asan       # only ASan
#   tools/sanitise/run.sh valgrind   # only valgrind
#
# Exit status is 0 only if at least one checker ran, every check it made
# passed, and every control behaved the way it was built to behave.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# NOT `set -e`.
#
# This script's job is to run programs that fail on purpose, and errexit is the
# wrong instrument for that. It also gave no protection where the risk actually
# was: inside a function invoked as `f || x` bash suppresses errexit entirely,
# which is how the previous version of this script sailed past three failed
# link commands and went on to run binaries that had never been created. Every
# command whose status matters is checked explicitly below, which is what
# errexit was being asked to do badly.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/tools/sanitise"
BUILD="$HERE/build"
SMOKE_C="$ROOT/crates/conform-ffi/tests/smoke/conform_smoke.c"
CONTROL_C="$HERE/negative_control.c"
LEAK_C="$HERE/leak_control.c"
INCLUDE="$ROOT/crates/conform-ffi/include"
REGISTRY="$ROOT/specs.toml"
DOCUMENT="$ROOT/crates/conform-odcs/tests/fixtures/faulty-many-faults.yaml"
CC="${CC:-cc}"

WANT="${1:-all}"

# --------------------------------------------------------------------------
# The verdict, and why it is counted rather than returned
#
# This used to be `run_asan || failed=1`: the verdict was each arm's exit
# status, so every check inside an arm had to remember to `return 1` at the
# right moment and `return 0` at every other. That is a fragile way to add up
# a result, and it conflates the two things that must never be conflated here
# — a *check* failing, which is bad, and a *control* failing, which is the
# control doing its job.
#
# So the arms record facts and the verdict is computed from them. A control
# that fires records a pass; the non-zero exit status it necessarily has never
# reaches the verdict, because that status is the evidence and not the outcome.
# --------------------------------------------------------------------------
arms_run=0      # checkers that got as far as running something
checks_run=0    # individual assertions made
checks_failed=0 # of those, the ones that did not hold
notes=()        # measured facts that are neither a pass nor a failure

say() { printf '\n== %s\n' "$*"; }
pass() { printf '   ok   — %s\n' "$1"; }
note() {
    printf '   note — %s\n' "$1"
    notes+=("$1")
}

# fail <summary> [detail...]
fail() {
    checks_failed=$((checks_failed + 1))
    printf '   FAIL — %s\n' "$1"
    shift
    local line
    for line in "$@"; do printf '          %s\n' "$line"; done
}

# Indent a log under a failure, so a report reads as a report rather than as
# something interleaved with this script's own output.
quote_log() {
    if [ -s "$1" ]; then
        sed 's/^/          | /' "$1"
    else
        printf '          | (the program produced no output at all)\n'
    fi
}

# build <output> <source> [link arguments...]
#
# The source comes FIRST and the libraries after it, which is not a matter of
# style. GNU ld walks the command line once and pulls members out of an archive
# only to satisfy undefined symbols it has *already* seen, so an archive placed
# before the object that needs it contributes nothing at all.
#
# That is exactly how this script failed on Linux while passing on macOS, whose
# linker resolves archives regardless of position: every `conform_*` symbol came
# back undefined, all three instrumented binaries failed to link, and the script
# — which was not checking whether they had linked — went on to run files that
# did not exist. Hence also the `checks_run` accounting here: a build failure is
# a loud, counted failure, not a silent precondition.
build() {
    local output=$1 source=$2
    shift 2
    checks_run=$((checks_run + 1))
    local log="$BUILD/$(basename "$output").link.log"
    if "$CC" -g -std=c11 -I "$INCLUDE" "$source" "$@" -o "$output" >"$log" 2>&1; then
        return 0
    fi
    fail "$(basename "$output") did not link" \
        "Nothing below this point could mean anything without it."
    quote_log "$log"
    return 1
}

# expect_clean <name> <log> <report pattern> -- <command...>
#
# The program must exit 0 *and* the checker must not have reported anything.
# Both, because a checker can report a problem and still let the program exit 0:
# valgrind only fails the process because `--error-exitcode` tells it to, and a
# LeakSanitizer report arrives after main has already returned successfully.
expect_clean() {
    local name=$1 log=$2 pattern=$3
    shift 4 # the `--`
    checks_run=$((checks_run + 1))

    local status=0
    "$@" >"$log" 2>&1 || status=$?

    if [ "$status" -ne 0 ]; then
        fail "$name: exited $status under the checker"
        quote_log "$log"
        return 1
    fi
    if grep -qE "$pattern" "$log"; then
        fail "$name: exited 0, but the checker reported a problem"
        quote_log "$log"
        return 1
    fi
    pass "$name"
    return 0
}

# expect_caught <name> <log> <report pattern> -- <command...>
#
# A control. It is *supposed* to fail, and the only thing that proves the
# checker is watching is the checker's own report in the output.
#
# A non-zero exit status is deliberately not the test. A binary that was never
# built exits 127; a program that died for an unrelated reason exits non-zero
# too; both would sail past a check that only asked "did it fail?", and one of
# them did. So the pattern is the assertion and the status is at most
# corroboration — which is also why a control doing its job records a *pass*,
# and its exit status never reaches the verdict.
expect_caught() {
    local name=$1 log=$2 pattern=$3
    shift 4 # the `--`
    checks_run=$((checks_run + 1))

    local status=0
    "$@" >"$log" 2>&1 || status=$?

    if grep -qE "$pattern" "$log"; then
        pass "$name — caught: $(grep -m1 -E "$pattern" "$log" | sed 's/^ *//')"
        return 0
    fi
    if [ "$status" -eq 0 ]; then
        fail "$name: the control ran to completion and nothing was reported." \
            "The checker is not watching, so a clean run in this configuration" \
            "would prove nothing at all."
    else
        fail "$name: the control failed (exit $status) with no report from the checker." \
            "That is not the failure it was built to cause, so it is evidence of a" \
            "broken harness rather than of a working checker."
    fi
    quote_log "$log"
    return 1
}

# probe_caught <name> <log> <report pattern> -- <command...>
#
# Like `expect_caught`, but a miss is a measured fact rather than a failure.
# Used where the capability genuinely may not exist on the platform and the
# honest answer is to say which half of a clean run was actually covered.
probe_caught() {
    local name=$1 log=$2 pattern=$3
    shift 4 # the `--`

    "$@" >"$log" 2>&1
    if grep -qE "$pattern" "$log"; then
        pass "$name — reported, so this checker is looking for leaks too"
        return 0
    fi
    note "$name was NOT reported: this checker is not looking for leaks on this platform, so the clean run above covers invalid accesses and not unreleased memory. The valgrind arm is where the leak claim gets made."
    return 0
}

# --------------------------------------------------------------------------
# Reading cargo's answers, which are another program's text
#
# Asked for rather than assumed. An earlier version hard-coded
# `$ROOT/target/$target/debug/libconform_ffi.a` and linked nothing beside it,
# which is two assumptions that happened to hold on one machine — and neither
# survives a `CARGO_TARGET_DIR`, a different profile, or a platform whose Rust
# static libraries need `-lpthread -ldl -lm` to link at all.
#
# Then read with more care than the version after that. This script used to
# scrape rustc's *rendered* note out of stderr:
#
#     note: native-static-libs: -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
#
# GitHub Actions sets `CARGO_TERM_COLOR: always`, so on CI that line is
# wrapped in ANSI and the trailing reset lands inside the last token. The
# linker was handed `-lc` followed by an escape sequence and said, accurately,
# `cannot find -lc^[[0m`.
#
# Two things changed. `--message-format=json` puts that note on *stdout* as a
# structured record whose `message` field carries the sentence undecorated,
# whatever `CARGO_TERM_COLOR` says — the colour lives in a separate `rendered`
# field this script never takes a value from. And every token that will become
# a linker argument is then checked anyway, and refused rather than repaired if
# it holds anything outside a small allow-list.
#
# `check_the_parser` below runs that guard against fixtures on every
# invocation, because a guard nobody exercises is a comment.
# --------------------------------------------------------------------------

# The staticlib path out of a `--message-format=json` stream.
archive_from() {
    sed -n 's/.*"filenames":\[\([^]]*\)\].*/\1/p' "$1" |
        tr ',' '\n' | tr -d '"' | grep -E '\.(a|lib)$' | head -1
}

# The cdylib path out of a `--message-format=json` stream.
cdylib_from() {
    sed -n 's/.*"filenames":\[\([^]]*\)\].*/\1/p' "$1" |
        tr ',' '\n' | tr -d '"' | grep -E '\.(so|dylib|dll)$' | head -1
}

# The `native-static-libs` note out of the STRUCTURED `message` field of a
# `compiler-message` record — never out of its `rendered` sibling, and never
# out of stderr.
#
# The pattern names `"message":"native-static-libs: ` exactly, which the
# rendered copy cannot match: rendered text reads `note: native-static-libs: `
# and the outer `"message"` key holds an object rather than a string.
native_from() {
    sed -n 's/.*"message":"native-static-libs: \([^"]*\)".*/\1/p' "$1" | head -1
}

# Whether a token is safe to hand to a linker.
#
# A character allow-list rather than a pattern per flag shape, because the
# shapes vary by platform and the characters do not: `-lpthread` on Linux,
# `-framework CoreFoundation` on macOS, bare `kernel32.lib` on Windows,
# absolute paths anywhere. What none of them contain is a control character, a
# quote, a space or a shell metacharacter.
is_safe_token() {
    case "$1" in
        "") return 1 ;;
        *[!A-Za-z0-9._/+:=,-]*) return 1 ;;
        *) return 0 ;;
    esac
}

# A token as it can be put in a message without doing to the reader's terminal
# what it would have done to the linker. A script that refuses an escape
# sequence and then echoes it has protected nothing.
printable() {
    printf '%s' "$1" | LC_ALL=C tr -c '[:print:]' '?'
}

# Filled by `read_native_libs`.
NATIVE_FLAGS=()
NATIVE_PROBLEM=""

# read_native_libs <cargo json stdout file>
#
# Sets `NATIVE_FLAGS` on success. On failure sets `NATIVE_PROBLEM` and leaves
# `NATIVE_FLAGS` empty, so a caller that ignored the status would link nothing
# rather than link something unexamined.
read_native_libs() {
    NATIVE_FLAGS=()
    NATIVE_PROBLEM=""

    local note token
    note="$(native_from "$1")"
    if [ -z "$note" ]; then
        NATIVE_PROBLEM="cargo's message stream carried no structured native-static-libs note"
        return 1
    fi

    for token in $note; do
        if ! is_safe_token "$token"; then
            NATIVE_FLAGS=()
            NATIVE_PROBLEM="a library flag holds a character this script will not pass to a linker: $(printable "$token")"
            return 1
        fi
        NATIVE_FLAGS+=("$token")
    done

    if [ "${#NATIVE_FLAGS[@]}" -eq 0 ]; then
        NATIVE_PROBLEM="the native-static-libs note named no libraries"
        return 1
    fi
    return 0
}

# --------------------------------------------------------------------------
# The harness's own parser, checked before anything trusts it
#
# Two fixtures and two counted checks, on every invocation. The escape in the
# second is the one that actually reached a linker, and it is present as a real
# `ESC` byte rather than as a description of one.
# --------------------------------------------------------------------------
check_the_parser() {
    say "the harness's own parser"
    local fixture="$BUILD/parser-fixture.json"

    printf '%s\n' \
        '{"reason":"compiler-message","message":{"level":"note","message":"native-static-libs: -lgcc_s -lpthread -lm -ldl -lc","rendered":"note: whatever"}}' \
        >"$fixture"
    checks_run=$((checks_run + 1))
    if read_native_libs "$fixture" && [ "${NATIVE_FLAGS[*]}" = "-lgcc_s -lpthread -lm -ldl -lc" ]; then
        pass "a clean note yields exactly the flags it names, in order"
    else
        fail "the parser could not read a clean note" \
            "problem: ${NATIVE_PROBLEM:-none reported}"
    fi

    # A real ESC byte, inserted by printf so that reading this file is not an
    # exercise in trusting your terminal.
    printf '{"reason":"compiler-message","message":{"level":"note","message":"native-static-libs: -lpthread -ldl -lc\033[0m"}}\n' \
        >"$fixture"
    checks_run=$((checks_run + 1))
    if read_native_libs "$fixture"; then
        fail "the parser accepted a library flag carrying an ANSI escape" \
            "It would have gone to the linker. That is the defect this check exists for," \
            "and it is why the flags are refused rather than cleaned up." \
            "got: $(printable "${NATIVE_FLAGS[*]}")"
    else
        pass "a flag carrying an ANSI escape is refused — $NATIVE_PROBLEM"
    fi

    # And the same escape as JSON would have to carry it, six characters rather
    # than one byte, because the backslash is outside the allow-list too.
    printf '%s\n' \
        '{"reason":"compiler-message","message":{"level":"note","message":"native-static-libs: -lpthread -lc\u001b[0m"}}' \
        >"$fixture"
    checks_run=$((checks_run + 1))
    if read_native_libs "$fixture"; then
        fail "the parser accepted a JSON-escaped ANSI sequence" \
            "got: $(printable "${NATIVE_FLAGS[*]}")"
    else
        pass "a JSON-escaped ANSI sequence is refused too"
    fi
}

mkdir -p "$BUILD"

# --------------------------------------------------------------------------
# AddressSanitizer
#
# Rust's ASan needs a nightly toolchain and the runtime that ships with it.
# The C side is compiled *without* `-fsanitize=address`: Apple's clang and
# rustc carry different, mutually exclusive ASan runtimes (the object files
# reference differently-versioned `__asan_version_mismatch_check_*` symbols),
# and linking both into one program fails. What that costs is instrumentation
# of loads and stores in the C program's own frames; what it keeps is the whole
# heap, because the sanitiser's malloc/free interceptors are process-wide once
# its runtime is loaded. Every allocation this library hands a caller is on
# that heap, which is where an FFI ownership bug lives.
#
# The negative control is what turns that paragraph from a claim into a test.
# --------------------------------------------------------------------------
run_asan() {
    if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
        echo "asan: skipped — no nightly toolchain (rustup toolchain install nightly)"
        return
    fi

    local target sysroot rtdir runtime
    target="$(rustc -vV | sed -n 's/^host: //p')"

    # `rustc -vV` is not affected by `CARGO_TERM_COLOR`, so this is a guard
    # against a class rather than against a known bug — but the triple becomes
    # a command-line argument two lines below, and the last thing this script
    # put on a command line without looking at it was an ANSI escape.
    #
    # Only the triple gets the token guard. The paths below do not: a home
    # directory may legitimately contain a space, and they are checked by
    # having to exist instead.
    if ! is_safe_token "$target"; then
        say "AddressSanitizer"
        arms_run=$((arms_run + 1))
        checks_run=$((checks_run + 1))
        fail "the host triple is not a triple: $(printable "$target")"
        return
    fi

    sysroot="$(rustup run nightly rustc --print sysroot)"
    rtdir="$sysroot/lib/rustlib/$target/lib"
    runtime="$(ls "$rtdir"/librustc-*_rt.asan.* 2>/dev/null | head -1)"

    if [ -z "$runtime" ]; then
        echo "asan: skipped — the nightly toolchain has no ASan runtime for $target"
        echo "      (rustup component add rust-src --toolchain nightly, or use a host that has one)"
        return
    fi

    say "AddressSanitizer ($target, runtime $(basename "$runtime"))"
    arms_run=$((arms_run + 1))

    # One invocation, two answers: where cargo put the archive, and which
    # system libraries this platform needs alongside a Rust `staticlib`.
    local out err
    out="$BUILD/asan-build.stdout"
    err="$BUILD/asan-build.stderr"
    checks_run=$((checks_run + 1))
    if ! RUSTFLAGS="-Zsanitizer=address" CARGO_TERM_COLOR=never cargo +nightly rustc \
        -p conform-ffi --lib --target "$target" --crate-type staticlib \
        --message-format=json --color=never \
        -- --print native-static-libs >"$out" 2>"$err"; then
        fail "the instrumented library did not build"
        quote_log "$err"
        echo "          | (structured diagnostics are in $out)"
        return
    fi

    local archive
    archive="$(archive_from "$out")"
    if [ -z "$archive" ] || [ ! -f "$archive" ]; then
        fail "cargo reported no usable staticlib for the instrumented build"
        quote_log "$out"
        return
    fi

    # From stdout now, not stderr, and from the structured field rather than a
    # rendered one — see the note above `native_from`. `read_native_libs` also
    # refuses anything it would not pass to a linker, and leaves the list empty
    # if it does, so there is no way past this point with an unexamined flag.
    #
    # Checked rather than defaulted for a second reason too: bash 3.2, which
    # macOS still ships, treats `"${empty[@]}"` as an unbound variable under
    # `set -u` and aborts. Failing here with a sentence beats aborting three
    # lines later with `native[@]: unbound variable`.
    checks_run=$((checks_run + 1))
    if ! read_native_libs "$out"; then
        fail "the library flags could not be read" "$NATIVE_PROBLEM"
        quote_log "$err"
        return
    fi
    pass "the library flags read clean: ${NATIVE_FLAGS[*]}"

    local link=("$archive" "$runtime" "${NATIVE_FLAGS[@]}" -Wl,-rpath,"$rtdir")
    build "$BUILD/smoke_asan" "$SMOKE_C" "${link[@]}" || return
    build "$BUILD/control_asan" "$CONTROL_C" "${link[@]}" || return
    build "$BUILD/leak_asan" "$LEAK_C" "${link[@]}" || return

    # The control first. If a deliberate double free goes unnoticed, the
    # sanitiser is not watching and nothing below this line would mean
    # anything.
    #
    # It is reported as a use-after-free rather than as a double free, and that
    # is the right answer: `conform_string_free` reconstitutes a `CString`,
    # which reads the string's length before releasing it, so the second call
    # reads freed memory a moment before it would have freed it twice. The read
    # is inside instrumented Rust, so it is the one that trips.
    echo "-- negative control: a double free, which MUST be caught"
    expect_caught "double free" "$BUILD/control.log" \
        'ERROR: AddressSanitizer' -- \
        "$BUILD/control_asan" "$REGISTRY"

    echo "-- the smoke test, which must be clean"
    expect_clean "smoke test" "$BUILD/smoke.log" \
        'ERROR: (AddressSanitizer|LeakSanitizer)' -- \
        "$BUILD/smoke_asan" "$REGISTRY" "$DOCUMENT"

    # Whether *leaks* are looked for is a separate question from whether
    # invalid accesses are, and the answer is platform-dependent. Both halves
    # of that were measured rather than assumed:
    #
    #   - on aarch64-unknown-linux-gnu, LeakSanitizer is part of ASan and
    #     reports the deliberate leak;
    #   - on aarch64-apple-darwin it is absent, and the same deliberate leak
    #     runs to completion unreported with `detect_leaks=1` explicitly set.
    #
    # So the assertion is chosen by platform. Where leak checking is expected,
    # a miss is a broken harness and fails; where it is known not to exist, a
    # miss is a fact about the host, and saying so is more use to a reader than
    # either failing or keeping quiet — it tells them which half of "ASan
    # clean" this run covered, and sends them to the valgrind arm for the
    # other half.
    local leak_check=probe_caught
    case "$target" in
        *-linux-*) leak_check=expect_caught ;;
    esac

    echo "-- leak control: a deliberate leak ($leak_check on $target)"
    "$leak_check" "the deliberate leak" "$BUILD/leak.log" \
        'ERROR: LeakSanitizer|detected memory leaks' -- \
        env ASAN_OPTIONS="detect_leaks=1" "$BUILD/leak_asan" "$REGISTRY"
}

# --------------------------------------------------------------------------
# valgrind
#
# Needs no instrumentation at all, which is its great advantage here: it runs
# over the ordinary build, C frames included, with no runtime to match against
# rustc's. It is also not available on Apple silicon, which is why this script
# has two arms rather than one.
#
# This arm links the `cdylib`, and is the only thing in the repository that
# does: `crates/conform-ffi/tests/a_c_program_can_link_and_validate.rs` links
# the `staticlib`, and every other test file there links the `rlib`. One linker
# apiece for the three declared crate-types, and none of the three relying on
# another having run first.
# --------------------------------------------------------------------------
run_valgrind() {
    if ! command -v valgrind >/dev/null 2>&1; then
        echo "valgrind: skipped — not installed on this machine"
        return
    fi

    say "valgrind ($(valgrind --version))"
    arms_run=$((arms_run + 1))

    local out err
    out="$BUILD/valgrind-build.stdout"
    err="$BUILD/valgrind-build.stderr"
    checks_run=$((checks_run + 1))
    if ! CARGO_TERM_COLOR=never cargo rustc -p conform-ffi --lib --crate-type cdylib \
        --message-format=json --color=never >"$out" 2>"$err"; then
        fail "the library did not build"
        quote_log "$err"
        echo "          | (structured diagnostics are in $out)"
        return
    fi

    local library
    library="$(cdylib_from "$out")"
    if [ -z "$library" ] || [ ! -f "$library" ]; then
        fail "cargo reported no usable cdylib"
        quote_log "$out"
        return
    fi
    local libdir
    libdir="$(dirname "$library")"

    local link=(-L "$libdir" -lconform_ffi -Wl,-rpath,"$libdir")
    build "$BUILD/smoke_valgrind" "$SMOKE_C" "${link[@]}" || return
    build "$BUILD/leak_valgrind" "$LEAK_C" "${link[@]}" || return

    # `--error-exitcode` so that a report is a failure rather than a paragraph
    # nobody reads. Leak checking is on: a boundary whose entire contract is
    # "call this function to give the allocation back" has to be held to it.
    local vg=(valgrind --error-exitcode=99 --leak-check=full
        --show-leak-kinds=definite,possible --errors-for-leak-kinds=definite)

    # The control first, for the same reason as in the ASan arm. Unlike ASan's,
    # valgrind's leak check is not optional, so a miss here is a broken harness
    # rather than a property of the host — `expect_caught`, not `probe_caught`.
    echo "-- leak control: a deliberate leak, which MUST be caught"
    expect_caught "the deliberate leak" "$BUILD/leak.log" \
        'definitely lost: [1-9]|possibly lost: [1-9]' -- \
        "${vg[@]}" "$BUILD/leak_valgrind" "$REGISTRY"

    echo "-- the smoke test, which must be clean"
    expect_clean "smoke test" "$BUILD/valgrind.log" \
        'definitely lost: [1-9]|ERROR SUMMARY: [1-9]' -- \
        "${vg[@]}" "$BUILD/smoke_valgrind" "$REGISTRY" "$DOCUMENT"
}

# Before either arm, because both of them depend on this script being able to
# read another program's output without being fooled by it, and because the one
# time it could not, the failure landed two steps downstream in a linker.
check_the_parser

case "$WANT" in
    all)
        run_asan
        run_valgrind
        ;;
    asan) run_asan ;;
    valgrind) run_valgrind ;;
    *)
        echo "usage: $0 [all|asan|valgrind]" >&2
        exit 2
        ;;
esac

echo
if [ "${#notes[@]}" -ne 0 ]; then
    echo "Measured, and neither a pass nor a failure:"
    printf '  - %s\n' "${notes[@]}"
    echo
fi

if [ "$arms_run" -eq 0 ]; then
    echo "NOTHING RAN. Every checker this script knows about skipped itself, so"
    echo "this machine has verified nothing. That is a failure, not a pass:"
    echo "an exit code of 0 here would be a claim nobody made."
    exit 1
fi
if [ "$checks_failed" -ne 0 ]; then
    echo "FAILED — $checks_failed of $checks_run checks across $arms_run checker(s)."
    exit 1
fi
echo "OK — $arms_run checker(s), $checks_run checks, all as designed."
