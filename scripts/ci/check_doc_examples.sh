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

examples=()
while IFS= read -r f; do
  examples+=("$f")
done < <(find docs/examples -maxdepth 1 -name '*.x07.json' -print | sort)

if [[ "${#examples[@]}" -eq 0 ]]; then
  echo "ERROR: no docs/examples/*.x07.json files found" >&2
  exit 2
fi

for f in "${examples[@]}"; do
  if ! "$x07_bin" lint --input "$f" >/dev/null; then
    echo "ERROR: docs example failed lint: $f" >&2
    "$x07_bin" lint --input "$f" >&2 || true
    exit 1
  fi
done

echo "ok: docs/examples/*.x07.json lint"

