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

step "published spec sync"
"$python_bin" scripts/sync_published_spec.py --check

step "genpack error-codes completeness"
"$python_bin" scripts/check_genpack_error_codes.py --check

step "tool JSON contracts"
"$python_bin" scripts/ci/check_tool_json_contracts.py

if [[ "${X07_ENABLE_GENPACK_SDK_CHECKS:-0}" == "1" ]]; then
  step "genpack sdk integration"
  ./scripts/ci/check_genpack_sdk.sh
else
  echo
  echo "==> genpack sdk integration (skipped; set X07_ENABLE_GENPACK_SDK_CHECKS=1 to enable)"
fi

step "cargo fmt --check"
cargo fmt --check

step "build runner binaries (for x07 run tests)"
cargo build -p x07-host-runner -p x07-os-runner

step "stage native math backend"
./scripts/ci/ensure_math_backend.sh

step "stage native stream-xf backend"
./scripts/ci/ensure_stream_xf_backend.sh

step "cargo test"
cargo test

step "monomorphization map determinism"
"$python_bin" scripts/check_monomorphization_map.py

step "generics intrinsics coherence"
"$python_bin" scripts/check_generics_intrinsics.py --check

step "cargo clippy --all-targets -- -D warnings"
cargo clippy --all-targets -- -D warnings

step "stream plugins smoke"
./scripts/ci/check_stream_plugins_smoke.sh

step "pkg contracts"
"$python_bin" scripts/check_pkg_contracts.py --check

step "skills"
./scripts/ci/check_skills.sh

step "check external packages lock"
./scripts/ci/check_external_packages_lock.sh

step "capabilities coherence"
"$python_bin" scripts/check_capabilities_coherence.py --check

step "canary gate"
./scripts/ci/check_canaries.sh

step "OS-world external packages smoke"
./scripts/ci/check_external_packages_os_smoke.sh

step "bundle smoke (native executable, no toolchain)"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    echo "ERROR: native Windows is not supported (WSL2 only)." >&2
    echo "hint: run this gate inside WSL2 (Ubuntu recommended): ./scripts/ci/check_all.sh" >&2
    exit 2
    ;;
  *)
    # On Linux CI, prefer the stronger "no toolchain installed" check via docker.
    # On WSL2, docker is typically unavailable; force the local fallback.
    if [[ "$(uname -s)" == "Linux" ]] && grep -qi microsoft /proc/version 2>/dev/null; then
      X07_BUNDLE_SMOKE_DOCKER=0 ./scripts/ci/check_bundle_smoke.sh
    else
      ./scripts/ci/check_bundle_smoke.sh
    fi
    ;;
esac

echo
echo "ok: all checks passed"
