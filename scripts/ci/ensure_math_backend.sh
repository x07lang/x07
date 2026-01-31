#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

# Build the native math backend if it isn't already staged into deps/.
if [[ ! -f "deps/x07/include/x07_math_abi_v1.h" ]] || \
   [[ ! -f "deps/x07/libx07_math.a" && ! -f "deps/x07/x07_math.lib" ]]; then
  ./scripts/build_ext_math.sh >/dev/null
fi

