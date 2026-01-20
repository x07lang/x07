#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

step() {
  echo
  echo "==> $*"
}

step "check tools"
./scripts/ci/check_tools.sh

step "agent surface sanity"
./scripts/ci/check_agent_surface_sanity.sh

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

step "policy: governance files"
"$python_bin" scripts/check_governance_files.py

step "policy: trademark policy present"
test -f TRADEMARKS.md

step "policy: release docs present"
test -f docs/releases.md
test -f docs/versioning.md
test -f docs/stability.md

step "licenses"
./scripts/ci/check_licenses.sh

step "release manifest (check)"
"$python_bin" scripts/build_release_manifest.py --check

step "cargo fmt --check"
cargo fmt --check

step "cargo test"
cargo test

step "cargo clippy --all-targets -- -D warnings"
cargo clippy --all-targets -- -D warnings

step "pkg contracts"
"$python_bin" scripts/check_pkg_contracts.py --check

step "skills"
./scripts/ci/check_skills.sh

step "check external packages lock"
./scripts/ci/check_external_packages_lock.sh

step "canary gate"
./scripts/ci/check_canaries.sh

step "OS-world external packages smoke"
./scripts/ci/check_external_packages_os_smoke.sh

echo
echo "ok: all checks passed"
