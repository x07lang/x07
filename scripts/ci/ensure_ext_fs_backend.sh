#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

# Build the native ext-fs backend if it isn't already staged into deps/.
if [[ ! -f "deps/x07/include/x07_ext_fs_abi_v1.h" ]] || \
   [[ ! -f "deps/x07/libx07_ext_fs.a" && ! -f "deps/x07/x07_ext_fs.lib" ]]; then
  ./scripts/build_ext_fs.sh >/dev/null
fi

