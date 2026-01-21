#!/usr/bin/env bash
set -euo pipefail

# Builds the native ext-fs backend static library and stages it into deps/.
#
# Expected consumers:
# - x07c link step should add deps/x07/libx07_ext_fs.a (or .lib on MSVC)
# - generated C can include deps/x07/include/x07_ext_fs_abi_v1.h

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# Force a deterministic, repo-root target dir so downstream scripts can find artifacts.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"

TARGET_TRIPLE=""
TARGET_RELEASE_DIR="$ROOT_DIR/target/release"

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    # When the end-user/CI config selects a GNU-like C compiler (gcc/clang),
    # build the native backend as windows-gnu so it can be linked by that toolchain.
    # (The default Rust toolchain on GitHub Windows runners is MSVC, which produces *.lib
    # that may not be linkable by mingw gcc in the agent gate.)
    cc="${X07_CC:-}"
    cc_lc="$(printf '%s' "$cc" | tr '[:upper:]' '[:lower:]')"
    if [[ -n "$cc_lc" && "$cc_lc" != *cl.exe && "$cc_lc" != *clang-cl* ]]; then
      TARGET_TRIPLE="x86_64-pc-windows-gnu"
      TARGET_RELEASE_DIR="$ROOT_DIR/target/$TARGET_TRIPLE/release"
      if command -v rustup >/dev/null 2>&1; then
        rustup target add "$TARGET_TRIPLE" >/dev/null
      else
        echo "ERROR: rustup is required to build native backends for $TARGET_TRIPLE" >&2
        exit 2
      fi
    fi
    ;;
esac

(
  cd "$ROOT_DIR"
  if [[ -n "$TARGET_TRIPLE" ]]; then
    cargo build --manifest-path crates/x07-ext-fs-native/Cargo.toml --release --target "$TARGET_TRIPLE"
  else
    cargo build --manifest-path crates/x07-ext-fs-native/Cargo.toml --release
  fi
)

LIB_CANDIDATES=(
  "$TARGET_RELEASE_DIR/libx07_ext_fs.a"
  "$TARGET_RELEASE_DIR/x07_ext_fs.lib"
  "$TARGET_RELEASE_DIR/libx07_ext_fs.lib"
)

LIB_PATH=""
for c in "${LIB_CANDIDATES[@]}"; do
  if [[ -f "$c" ]]; then
    LIB_PATH="$c"
    break
  fi
done

if [[ -z "$LIB_PATH" ]]; then
  echo "ERROR: could not find built x07_ext_fs static library under target/release/." >&2
  echo "Tried: ${LIB_CANDIDATES[*]}" >&2
  exit 2
fi

DEPS_DIR="$ROOT_DIR/deps/x07"
mkdir -p "$DEPS_DIR/include"

cp -f "$ROOT_DIR/crates/x07c/include/x07_ext_fs_abi_v1.h" \
  "$DEPS_DIR/include/x07_ext_fs_abi_v1.h"

STAGED_LIB=""
if [[ "$LIB_PATH" == *.a ]]; then
  STAGED_LIB="$DEPS_DIR/libx07_ext_fs.a"
  cp -f "$LIB_PATH" "$STAGED_LIB"
  if [[ -n "$TARGET_TRIPLE" ]]; then
    cp -f "$LIB_PATH" "$DEPS_DIR/x07_ext_fs.lib"
  fi
else
  STAGED_LIB="$DEPS_DIR/x07_ext_fs.lib"
  cp -f "$LIB_PATH" "$STAGED_LIB"
fi

echo "Staged:"
echo "  $DEPS_DIR/include/x07_ext_fs_abi_v1.h"
echo "  $STAGED_LIB"
