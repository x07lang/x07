#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

python3 scripts/check_stdlib_package_manifests.py --root stdlib
python3 scripts/generate_stdlib_lock.py --check
python3 scripts/generate_stdlib_lock.py --stdlib-root stdlib/os --out stdlib.os.lock --check

echo "ok: stdlib package manifests + locks up to date"
