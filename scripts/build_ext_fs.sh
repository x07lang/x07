#!/usr/bin/env bash
set -euo pipefail

# Builds the native ext-fs backend static library and stages it into deps/.
#
# Expected consumers:
# - x07c link step should add deps/x07/libx07_ext_fs.a (or .lib on MSVC)
# - generated C can include deps/x07/include/x07_ext_fs_abi_v1.h

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

cargo_cmd="${X07_CARGO:-cargo}"
user_target_dir="${CARGO_TARGET_DIR:-}"

host_target_dir=""
cargo_target_dir_env=""
if [[ "$cargo_cmd" == "cross" ]]; then
  if [[ -n "$user_target_dir" ]]; then
    if [[ "$user_target_dir" == /* ]]; then
      case "$user_target_dir" in
        "$ROOT_DIR"/*)
          cargo_target_dir_env="${user_target_dir#"$ROOT_DIR"/}"
          host_target_dir="$user_target_dir"
          ;;
        *)
          echo "ERROR: cross build requires CARGO_TARGET_DIR under repo root (got: $user_target_dir)" >&2
          exit 2
          ;;
      esac
    else
      cargo_target_dir_env="$user_target_dir"
      host_target_dir="$ROOT_DIR/$user_target_dir"
    fi
  else
    cargo_target_dir_env="target"
    host_target_dir="$ROOT_DIR/target"
  fi
else
  if [[ -n "$user_target_dir" ]]; then
    if [[ "$user_target_dir" == /* ]]; then
      host_target_dir="$user_target_dir"
    else
      host_target_dir="$ROOT_DIR/$user_target_dir"
    fi
  else
    host_target_dir="$ROOT_DIR/target"
  fi
  cargo_target_dir_env="$host_target_dir"
fi

TARGET_TRIPLE=""
TARGET_RELEASE_DIR="$host_target_dir/release"

override_target="${X07_CARGO_TARGET:-}"
if [[ -n "$override_target" ]]; then
  TARGET_TRIPLE="$override_target"
  TARGET_RELEASE_DIR="$host_target_dir/$TARGET_TRIPLE/release"
fi

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    if [[ -n "$TARGET_TRIPLE" ]]; then
      break
    fi
    # When the end-user/CI config selects a GNU-like C compiler (gcc/clang),
    # build the native backend as windows-gnu so it can be linked by that toolchain.
    # (The default Rust toolchain on GitHub Windows runners is MSVC, which produces *.lib
    # that may not be linkable by mingw gcc in the agent gate.)
    cc="${X07_CC:-}"
    cc_lc="$(printf '%s' "$cc" | tr '[:upper:]' '[:lower:]')"
    if [[ -n "$cc_lc" && "$cc_lc" != *cl.exe && "$cc_lc" != *clang-cl* ]]; then
      TARGET_TRIPLE="x86_64-pc-windows-gnu"
      TARGET_RELEASE_DIR="$host_target_dir/$TARGET_TRIPLE/release"
      if [[ "$cargo_cmd" == "cargo" || "$cargo_cmd" == */cargo ]] && command -v rustup >/dev/null 2>&1; then
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
    CARGO_TARGET_DIR="$cargo_target_dir_env" "$cargo_cmd" build --manifest-path crates/x07-ext-fs-native/Cargo.toml --release --target "$TARGET_TRIPLE"
  else
    CARGO_TARGET_DIR="$cargo_target_dir_env" "$cargo_cmd" build --manifest-path crates/x07-ext-fs-native/Cargo.toml --release
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
  if [[ "$TARGET_TRIPLE" == "x86_64-pc-windows-gnu" ]]; then
    cp -f "$LIB_PATH" "$DEPS_DIR/x07_ext_fs.lib"
  fi
else
  STAGED_LIB="$DEPS_DIR/x07_ext_fs.lib"
  cp -f "$LIB_PATH" "$STAGED_LIB"
fi

echo "Staged:"
echo "  $DEPS_DIR/include/x07_ext_fs_abi_v1.h"
echo "  $STAGED_LIB"
