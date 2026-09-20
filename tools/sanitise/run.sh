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
# Exit status is 0 only if at least one checker ran and all that ran were
# clean.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

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
# How many checkers actually ran, as opposed to skipped themselves. A script
# that reports success without having run anything is the failure mode this
# whole file exists to avoid.
ran=0
failed=0

say() { printf '\n== %s\n' "$*"; }

mkdir -p "$BUILD"

# --------------------------------------------------------------------------
# AddressSanitizer
#
# Rust's ASan needs a nightly toolchain and the runtime that ships with it.
# The C side is compiled *without* `-fsanitize=address`: Apple's clang and
# rustc carry different, mutually exclusive ASan runtimes (the object files
# reference differently-versioned `__asan_version_mismatch_check_*` symbols),
# and linking both into one program fails. What that costs is instrumentation
# of loads and stores in the smoke program's own frames; what it keeps is the
# whole heap, because the sanitiser's malloc/free interceptors are
# process-wide once its runtime is loaded. Every allocation this library hands
# a caller is on that heap, which is where an FFI ownership bug lives.
#
# The negative control is what turns that paragraph from a claim into a test.
# --------------------------------------------------------------------------
run_asan() {
    if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
        echo "asan: skipped — no nightly toolchain (rustup toolchain install nightly)"
        return 0
    fi

    local target sysroot rtdir runtime lib
    target="$(rustc -vV | sed -n 's/^host: //p')"
    sysroot="$(rustup run nightly rustc --print sysroot)"
    rtdir="$sysroot/lib/rustlib/$target/lib"
    runtime="$(ls "$rtdir"/librustc-*_rt.asan.* 2>/dev/null | head -1 || true)"

    if [ -z "$runtime" ]; then
        echo "asan: skipped — the nightly toolchain has no ASan runtime for $target"
        echo "      (rustup component add rust-src --toolchain nightly, or use a host that has one)"
        return 0
    fi

    say "AddressSanitizer ($target, runtime $(basename "$runtime"))"
    ran=$((ran + 1))

    RUSTFLAGS="-Zsanitizer=address" cargo +nightly build -p conform-ffi --target "$target"
    lib="$ROOT/target/$target/debug/libconform_ffi.a"
    [ -f "$lib" ] || { echo "asan: FAILED — $lib was not built"; return 1; }

    local link=("$CC" -g -std=c11 -I "$INCLUDE" "$lib" "$runtime" -Wl,-rpath,"$rtdir")

    "${link[@]}" "$SMOKE_C" -o "$BUILD/smoke_asan"
    "${link[@]}" "$CONTROL_C" -o "$BUILD/control_asan"
    "${link[@]}" "$LEAK_C" -o "$BUILD/leak_asan"

    # The control first. If a deliberate double free goes unnoticed, the
    # sanitiser is not watching and nothing below this line would mean
    # anything.
    #
    # It is reported as a use-after-free rather than as a double free, and
    # that is the right answer: `conform_string_free` reconstitutes a
    # `CString`, which reads the string's length before releasing it, so the
    # second call reads freed memory a moment before it would have freed it
    # twice. The read is inside instrumented Rust, so it is the one that
    # trips.
    echo "-- negative control (a double free, which MUST be caught)"
    if "$BUILD/control_asan" "$REGISTRY" >"$BUILD/control.log" 2>&1; then
        echo "asan: FAILED — the negative control exited 0, so the sanitiser is not live."
        echo "      A clean run in this configuration would prove nothing. Output:"
        sed 's/^/      /' "$BUILD/control.log"
        return 1
    fi
    if ! grep -q "AddressSanitizer" "$BUILD/control.log"; then
        echo "asan: FAILED — the negative control failed, but not with an ASan report."
        sed 's/^/      /' "$BUILD/control.log"
        return 1
    fi
    echo "   ok — $(grep -m1 'ERROR: AddressSanitizer' "$BUILD/control.log")"

    echo "-- the smoke test (which must be clean)"
    if ! "$BUILD/smoke_asan" "$REGISTRY" "$DOCUMENT" >"$BUILD/smoke.log" 2>&1; then
        echo "asan: FAILED — the smoke test did not pass under the sanitiser:"
        sed 's/^/      /' "$BUILD/smoke.log"
        return 1
    fi
    if grep -q "ERROR: AddressSanitizer" "$BUILD/smoke.log"; then
        echo "asan: FAILED — the smoke test exited 0 but the sanitiser reported:"
        sed 's/^/      /' "$BUILD/smoke.log"
        return 1
    fi
    echo "   ok — clean"

    # Whether *leaks* are being looked for is a separate question from whether
    # invalid accesses are, and the answer is platform-dependent. Measured
    # rather than assumed, and reported either way: a reader is entitled to
    # know which half of "ASan clean" this run actually covered.
    echo "-- leak control (a deliberate leak; whether it is seen is the question)"
    if ASAN_OPTIONS="${ASAN_OPTIONS:-}${ASAN_OPTIONS:+,}detect_leaks=1" \
        "$BUILD/leak_asan" "$REGISTRY" >"$BUILD/leak.log" 2>&1; then
        echo "   NOTE — the deliberate leak was NOT reported."
        echo "          LeakSanitizer is not active in this configuration, so the clean"
        echo "          run above covers invalid accesses and not unreleased memory."
        echo "          Run the valgrind arm on a Linux host for the other half."
    else
        echo "   ok — the deliberate leak was reported, so leak checking is live too"
    fi
    return 0
}

# --------------------------------------------------------------------------
# valgrind
#
# Needs no instrumentation at all, which is its great advantage here: it runs
# over the ordinary release-of-the-day build, C frames included, with no
# runtime to match against rustc's. It is also not available on Apple silicon,
# which is why this script has two arms rather than one.
# --------------------------------------------------------------------------
run_valgrind() {
    if ! command -v valgrind >/dev/null 2>&1; then
        echo "valgrind: skipped — not installed on this machine"
        return 0
    fi

    say "valgrind ($(valgrind --version))"
    ran=$((ran + 1))

    cargo build -p conform-ffi
    local libdir="$ROOT/target/debug"
    local lib
    for candidate in libconform_ffi.so libconform_ffi.dylib; do
        [ -f "$libdir/$candidate" ] && lib="$libdir/$candidate"
    done
    [ -n "${lib:-}" ] || { echo "valgrind: FAILED — no shared library in $libdir"; return 1; }

    "$CC" -g -std=c11 -I "$INCLUDE" "$SMOKE_C" \
        -L "$libdir" -lconform_ffi -Wl,-rpath,"$libdir" -o "$BUILD/smoke_valgrind"
    "$CC" -g -std=c11 -I "$INCLUDE" "$LEAK_C" \
        -L "$libdir" -lconform_ffi -Wl,-rpath,"$libdir" -o "$BUILD/leak_valgrind"

    local vg=(valgrind --error-exitcode=99 --leak-check=full
              --show-leak-kinds=definite,possible --errors-for-leak-kinds=definite)

    # The control first, for the same reason as in the ASan arm: a leak check
    # that cannot see a deliberate leak has nothing to say about an accidental
    # one. Unlike ASan's, valgrind's leak check is not optional, so a clean run
    # here is a failure of this script rather than a property of the host.
    echo "-- leak control (a deliberate leak, which MUST be caught)"
    if "${vg[@]}" "$BUILD/leak_valgrind" "$REGISTRY" >"$BUILD/leak.log" 2>&1; then
        echo "valgrind: FAILED — the deliberate leak was not reported, so this"
        echo "          invocation is not checking what it claims to check."
        sed 's/^/      /' "$BUILD/leak.log"
        return 1
    fi
    echo "   ok — reported"

    echo "-- the smoke test (which must be clean)"

    # `--error-exitcode` so that a report is a failure rather than a paragraph
    # nobody reads. Leak checking is on: a boundary whose entire contract is
    # "call this function to give the allocation back" has to be held to it.
    if ! "${vg[@]}" "$BUILD/smoke_valgrind" "$REGISTRY" "$DOCUMENT" \
        >"$BUILD/valgrind.log" 2>&1; then
        echo "valgrind: FAILED"
        sed 's/^/      /' "$BUILD/valgrind.log"
        return 1
    fi
    echo "   ok — clean"
    return 0
}

case "$WANT" in
    all)
        run_asan || failed=1
        run_valgrind || failed=1
        ;;
    asan) run_asan || failed=1 ;;
    valgrind) run_valgrind || failed=1 ;;
    *) echo "usage: $0 [all|asan|valgrind]" >&2; exit 2 ;;
esac

echo
if [ "$ran" -eq 0 ]; then
    echo "NOTHING RAN. Every checker this script knows about skipped itself, so"
    echo "this machine has verified nothing. That is a failure, not a pass:"
    echo "an exit code of 0 here would be a claim nobody made."
    exit 1
fi
if [ "$failed" -ne 0 ]; then
    echo "FAILED — $ran checker(s) ran and at least one was not clean."
    exit 1
fi
echo "OK — $ran checker(s) ran, all clean."
