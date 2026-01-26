#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

./scripts/ci/check_tools.sh >/dev/null

targets_py="./scripts/build_channels_json.py"
install_md="./docs/getting-started/install.md"

match() {
  local pat="$1"
  local path="$2"
  if command -v rg >/dev/null 2>&1; then
    rg -q "$pat" "$path"
  else
    grep -Eq "$pat" "$path"
  fi
}

if [[ ! -f "$targets_py" ]]; then
  echo "ERROR: missing channels builder: $targets_py" >&2
  exit 2
fi
if [[ ! -f "$install_md" ]]; then
  echo "ERROR: missing install docs: $install_md" >&2
  exit 2
fi

# Enforce that the install docs don't contradict the supported targets emitted by
# scripts/build_channels_json.py (Milestone 7).
if match "aarch64-unknown-linux-gnu" "$targets_py"; then
  if match "Linux ARM64.*require building from source" "$install_md"; then
    echo "ERROR: install docs claim Linux ARM64 requires source builds, but channels manifest includes aarch64-unknown-linux-gnu" >&2
    exit 1
  fi
  if ! match "Linux.*ARM64" "$install_md"; then
    echo "ERROR: install docs must list Linux ARM64 as supported when channels manifest includes aarch64-unknown-linux-gnu" >&2
    exit 1
  fi
fi

# x07up must be the canonical installer surface.
if ! match "x07up" "$install_md"; then
  echo "ERROR: install docs must mention x07up as the installer" >&2
  exit 1
fi

echo "ok: check_install_docs_targets"
