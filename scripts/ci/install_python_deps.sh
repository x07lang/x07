#!/usr/bin/env bash
set -euo pipefail

python="${X07_PYTHON:-python3}"

pip_install() {
  local tmp
  tmp="$(mktemp)"
  if "$python" -m pip "$@" 2>"$tmp"; then
    rm -f "$tmp"
    return 0
  fi

  if grep -q "externally-managed-environment" "$tmp" 2>/dev/null; then
    if "$python" -m pip "$@" --break-system-packages --ignore-installed; then
      rm -f "$tmp"
      return 0
    fi
  fi

  cat "$tmp" >&2 || true
  rm -f "$tmp"
  return 1
}

pip_install install 'jsonschema==4.25.1'
