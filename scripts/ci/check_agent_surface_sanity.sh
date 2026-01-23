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

for f in \
  spec/x07-project.schema.json \
  spec/x07-lock.schema.json \
  spec/x07-package.schema.json \
  spec/x07-capabilities.schema.json \
  spec/x07-website.package-index.schema.json \
  catalog/capabilities.json \
; do
  if [[ ! -f "$f" ]]; then
    echo "ERROR: missing required agent contract file: $f" >&2
    exit 1
  fi
done

echo "ok: agent surface sanity"
