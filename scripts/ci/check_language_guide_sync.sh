#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

tmp="$(mktemp)"
cleanup() {
  rm -f "$tmp"
}
trap cleanup EXIT

x07c_bin="$(./scripts/ci/find_x07c.sh)"
"$x07c_bin" guide >"$tmp"

if ! diff -u "$tmp" docs/spec/language-guide.md >&2; then
  echo "ERROR: docs/spec/language-guide.md is out of sync with x07c guide" >&2
  echo "Regen: cargo run -q -p x07c -- guide > docs/spec/language-guide.md" >&2
  exit 1
fi

skill_copy="skills/pack/.codex/skills/x07-language-guide/references/language-guide.md"
if ! diff -u "$tmp" "$skill_copy" >&2; then
  echo "ERROR: $skill_copy is out of sync with x07c guide" >&2
  echo "Regen: cargo run -q -p x07c -- guide > $skill_copy" >&2
  exit 1
fi

echo "ok: language guide in sync"
