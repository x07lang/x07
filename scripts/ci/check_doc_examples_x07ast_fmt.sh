#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "$x07_bin" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

files=()
while IFS= read -r -d '' f; do
  files+=("$f")
done < <(find docs/examples -name '*.x07.json' -type f -not -path '*/.*/*' -print0 | sort -z)

if [[ "${#files[@]}" -eq 0 ]]; then
  echo "ERROR: no docs/examples/**/*.x07.json files found" >&2
  exit 2
fi

ok=1
for f in "${files[@]}"; do
  if ! "$x07_bin" fmt --input "$f" --check >/dev/null; then
    ok=0
  fi
done

if [[ "$ok" -ne 1 ]]; then
  echo "ERROR: docs/examples x07AST files must be valid and formatted (run: x07 fmt --input docs/examples --write)" >&2
  exit 1
fi

echo "ok: docs/examples x07AST files formatted"

