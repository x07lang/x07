#!/usr/bin/env bash
set -euo pipefail

# CI entrypoint: build the native ext-fs backend and run its smoke suites.

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

source ./scripts/ci/lib_ext_packages.sh

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  elif command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  else
    python_bin="python"
  fi
fi

./scripts/build_ext_fs.sh >/dev/null

cargo build -p x07-host-runner >/dev/null
cargo build -p x07-os-runner >/dev/null

pick_runner() {
  local env_override="$1"
  local bin_name="$2"

  if [[ -n "$env_override" ]]; then
    echo "$env_override"
    return 0
  fi

  local candidates=(
    "$root/target/debug/$bin_name"
    "$root/target/debug/$bin_name.exe"
    "$root/target/release/$bin_name"
    "$root/target/release/$bin_name.exe"
  )
  for c in "${candidates[@]}"; do
    if [[ -x "$c" ]]; then
      echo "$c"
      return 0
    fi
  done

  echo "" >&2
  echo "ERROR: $bin_name not found under target/{debug,release}/ (set env override)" >&2
  echo "Tried:" >&2
  for c in "${candidates[@]}"; do
    echo "  $c" >&2
  done
  return 2
}

X07_HOST_RUNNER="$(pick_runner "${X07_HOST_RUNNER:-}" "x07-host-runner")"
X07_OS_RUNNER="$(pick_runner "${X07_OS_RUNNER:-}" "x07-os-runner")"

MODULE_ROOT="${X07_EXT_FS_MODULE_ROOT:-$(x07_ext_pkg_modules x07-ext-fs)}"
if [[ ! -d "$MODULE_ROOT" ]]; then
  echo "ERROR: ext-fs module root not found at $MODULE_ROOT" >&2
  exit 2
fi

run_suite() {
  local suite="$1"
  echo "[fs-smoke] running suite: $suite"
  "$python_bin" scripts/ci/run_smoke_suite.py \
    --suite "$suite" \
    --host-runner "$X07_HOST_RUNNER" \
    --os-runner "$X07_OS_RUNNER" \
    --module-root "$MODULE_ROOT"
}

run_suite "ci/suites/smoke/fs-os-smoke.json"
run_suite "ci/suites/smoke/fs-os-sandboxed-policy-deny-smoke.json"

echo "[fs-globwalk] running ext-path-glob-rs smoke (run-os)"
PATH_GLOB_MODULE_ROOT="$(x07_ext_pkg_modules x07-ext-path-glob-rs)"
GLOB_MODULE_ROOT="$(x07_ext_pkg_modules x07-ext-glob-rs)"
if [[ ! -d "$PATH_GLOB_MODULE_ROOT" ]]; then
  echo "ERROR: ext-path-glob-rs module root not found at $PATH_GLOB_MODULE_ROOT" >&2
  exit 2
fi
if [[ ! -d "$GLOB_MODULE_ROOT" ]]; then
  echo "ERROR: ext-glob-rs module root not found at $GLOB_MODULE_ROOT" >&2
  exit 2
fi
"$X07_OS_RUNNER" \
  --program "tests/external_os/fs_globwalk_smoke_ok/src/main.x07.json" \
  --world run-os \
  --module-root "$MODULE_ROOT" \
  --module-root "$PATH_GLOB_MODULE_ROOT" \
  --module-root "$GLOB_MODULE_ROOT" \
  | "$python_bin" scripts/ci/assert_run_os_ok.py "fs-globwalk" --expect OK

echo "[fs-smoke] OK"
