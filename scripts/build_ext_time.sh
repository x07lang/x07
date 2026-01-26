#!/usr/bin/env bash
set -euo pipefail

# Builds the native time backend static library and stages it into deps/.
#
# Expected consumers:
# - x07c link step should add deps/x07/libx07_time.a (or .lib on MSVC)
# - generated C can include deps/x07/include/x07_time_abi_v1.h

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

exec "$ROOT_DIR/scripts/build_ext_staticlib.sh" \
  --manifest crates/x07-time-native/Cargo.toml \
  --lib-name x07_time \
  --header crates/x07c/include/x07_time_abi_v1.h
