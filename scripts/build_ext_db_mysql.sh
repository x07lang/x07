#!/usr/bin/env bash
set -euo pipefail

# Builds the native ext-db-mysql backend static library and stages it into deps/.
#
# Expected consumers:
# - x07c link step should add deps/x07/libx07_ext_db_mysql.a (or .lib on MSVC)
# - generated C can include deps/x07/include/x07_ext_db_mysql_abi_v1.h

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# Force a deterministic, repo-root target dir so downstream scripts can find artifacts.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"

(
  cd "$ROOT_DIR"
  cargo build --manifest-path crates/x07-ext-db-mysql-native/Cargo.toml --release
)

LIB_CANDIDATES=(
  "$ROOT_DIR/target/release/libx07_ext_db_mysql.a"
  "$ROOT_DIR/target/release/x07_ext_db_mysql.lib"
  "$ROOT_DIR/target/release/libx07_ext_db_mysql.lib"
)

LIB_PATH=""
for c in "${LIB_CANDIDATES[@]}"; do
  if [[ -f "$c" ]]; then
    LIB_PATH="$c"
    break
  fi
done

if [[ -z "$LIB_PATH" ]]; then
  echo "ERROR: could not find built x07_ext_db_mysql static library under target/release/." >&2
  echo "Tried: ${LIB_CANDIDATES[*]}" >&2
  exit 2
fi

DEPS_DIR="$ROOT_DIR/deps/x07"
mkdir -p "$DEPS_DIR/include"

cp -f "$ROOT_DIR/crates/x07c/include/x07_ext_db_mysql_abi_v1.h" \
  "$DEPS_DIR/include/x07_ext_db_mysql_abi_v1.h"

STAGED_LIB=""
if [[ "$LIB_PATH" == *.a ]]; then
  STAGED_LIB="$DEPS_DIR/libx07_ext_db_mysql.a"
  cp -f "$LIB_PATH" "$STAGED_LIB"
else
  STAGED_LIB="$DEPS_DIR/x07_ext_db_mysql.lib"
  cp -f "$LIB_PATH" "$STAGED_LIB"
fi

echo "Staged:"
echo "  $DEPS_DIR/include/x07_ext_db_mysql_abi_v1.h"
echo "  $STAGED_LIB"

