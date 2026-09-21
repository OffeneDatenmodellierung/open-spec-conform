#!/usr/bin/env bash
#
# Build `conform-ffi`'s `wasm-bindgen` binding.
#
#   tools/wasm/build.sh [--target web|nodejs] [--out <dir>]
#
# Defaults: `--target web`, `--out website/dist/wasm`. The site generator looks
# for the artefacts there and will only claim an in-browser demo if it finds
# them — see `crates/conform-web/src/site.rs`.
#
# Outside the cargo workspace, like `tools/headergen` and `tools/sanitise` next
# door, because it is a build harness rather than a crate and neither
# `cargo test` nor `cargo deny` should have an opinion about it. The
# `wasm-bindgen` CLI it needs is a *tool*, installed here and never a
# dependency of any crate in this repository.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# Not `set -e`: this script decides whether each step worked and says so, and
# `tools/sanitise/run.sh` next door records why errexit is the wrong instrument
# for a script whose failures have to be reported rather than merely fatal.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TARGET="web"
OUT="$ROOT/website/dist/wasm"
WASM_TARGET="wasm32-unknown-unknown"

# The version of the CLI has to be the version of the crate. `wasm-bindgen`'s
# JavaScript glue and the custom sections the macro emits into the `.wasm` are
# one protocol with no compatibility promise across releases, and a mismatch
# fails loudly at generation time rather than subtly at runtime — which is the
# good outcome, and the reason this is read from the manifest rather than
# written down twice.
CRATE_MANIFEST="$ROOT/crates/conform-ffi/Cargo.toml"
WB_VERSION="$(sed -n 's/^wasm-bindgen = { version = "\([^"]*\)".*/\1/p' "$CRATE_MANIFEST")"

while [ $# -gt 0 ]; do
  case "$1" in
    --target) TARGET="${2:?--target needs web or nodejs}"; shift 2 ;;
    --out)    OUT="${2:?--out needs a directory}"; shift 2 ;;
    -h|--help) sed -n '2,12p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "tools/wasm/build.sh: unexpected argument $1" >&2; exit 2 ;;
  esac
done

fail() { echo "tools/wasm/build.sh: $*" >&2; exit 1; }

[ -n "$WB_VERSION" ] || fail "could not read the wasm-bindgen version out of $CRATE_MANIFEST"

echo "tools/wasm/build.sh: wasm-bindgen $WB_VERSION, target $TARGET, out $OUT"

# 1. The target. `rustup` is how CI and a developer machine both get it; a
#    build environment without rustup is expected to have installed it already,
#    so a missing target is reported by the compiler rather than guessed at
#    here.
if command -v rustup >/dev/null 2>&1; then
  rustup target add "$WASM_TARGET" >/dev/null 2>&1 || true
fi

# 2. The CLI, at exactly the crate's version. Installed into the cargo bin
#    directory, which is on PATH in CI and on a developer machine alike.
if ! command -v wasm-bindgen >/dev/null 2>&1 \
   || [ "$(wasm-bindgen --version 2>/dev/null)" != "wasm-bindgen $WB_VERSION" ]; then
  echo "tools/wasm/build.sh: installing wasm-bindgen-cli $WB_VERSION"
  cargo install wasm-bindgen-cli --version "$WB_VERSION" --locked \
    || fail "could not install wasm-bindgen-cli $WB_VERSION"
fi

# 3. The module. `--release` because a debug `.wasm` is several times the size
#    and nothing about this binding is worth debugging at the byte level from a
#    browser.
cargo build \
  --manifest-path "$CRATE_MANIFEST" \
  --features wasm \
  --target "$WASM_TARGET" \
  --release \
  || fail "the wasm build failed"

MODULE="$ROOT/target/$WASM_TARGET/release/conform_ffi.wasm"
[ -f "$MODULE" ] || fail "cargo reported success and produced no $MODULE"

# 4. The glue.
rm -rf "$OUT"
mkdir -p "$OUT" || fail "could not create $OUT"
wasm-bindgen --target "$TARGET" --out-dir "$OUT" "$MODULE" \
  || fail "wasm-bindgen could not generate the $TARGET binding"

# 5. Say what was produced, in bytes, because "it built" is not a measurement.
for artefact in "$OUT"/conform_ffi.js "$OUT"/conform_ffi_bg.wasm; do
  [ -f "$artefact" ] || fail "wasm-bindgen did not produce $artefact"
  echo "tools/wasm/build.sh: $(basename "$artefact") $(wc -c < "$artefact" | tr -d ' ') bytes"
done

echo "tools/wasm/build.sh: wrote $OUT"
