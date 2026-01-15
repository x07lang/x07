#!/usr/bin/env bash
set -euo pipefail

base_sha="${1:-}"
head_sha="${2:-}"

if [[ -z "$base_sha" || -z "$head_sha" ]]; then
  echo "usage: $0 <base_sha> <head_sha>" >&2
  exit 2
fi

missing=0

while read -r commit; do
  msg="$(git show -s --format=%B "$commit")"
  if ! grep -Eq '^Signed-off-by: .+ <[^>]+>$' <<<"$msg"; then
    echo "ERROR: commit missing Signed-off-by: $commit" >&2
    missing=1
  fi
done < <(git rev-list "${base_sha}..${head_sha}")

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

echo "ok: dco sign-off present"

