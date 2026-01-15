#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

missing=0

for f in LICENSE-APACHE LICENSE-MIT; do
  if [[ ! -f "$f" ]]; then
    echo "ERROR: missing $f" >&2
    missing=1
  fi
done

if [[ ! -f README.md ]]; then
  echo "ERROR: missing README.md" >&2
  missing=1
else
  if ! grep -q "^## License$" README.md; then
    echo "ERROR: README.md missing '## License' section" >&2
    missing=1
  fi
  if ! grep -q "LICENSE-APACHE" README.md; then
    echo "ERROR: README.md missing LICENSE-APACHE reference" >&2
    missing=1
  fi
  if ! grep -q "LICENSE-MIT" README.md; then
    echo "ERROR: README.md missing LICENSE-MIT reference" >&2
    missing=1
  fi
fi

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

echo "ok: licenses present"

