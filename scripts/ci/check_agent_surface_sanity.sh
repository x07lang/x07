#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

if [[ -e "deps/x07lang" ]]; then
  echo "ERROR: unexpected legacy artifacts directory exists: deps/x07lang" >&2
  exit 1
fi

symlinks="$(find tests -type l -print || true)"
if [[ -n "${symlinks}" ]]; then
  echo "ERROR: tests must not contain symlinks:" >&2
  echo "${symlinks}" >&2
  exit 1
fi

echo "ok: agent surface sanity"

