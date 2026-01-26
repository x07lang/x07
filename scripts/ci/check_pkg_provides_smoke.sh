#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

./scripts/ci/check_tools.sh >/dev/null

X07_BIN="${X07_BIN:-$(./scripts/ci/find_x07.sh)}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/src"

cat >"$tmp/x07.json" <<'JSON'
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "src/main.x07.json",
  "module_roots": ["src"],
  "lockfile": null
}
JSON

"$X07_BIN" pkg provides std.os.process --project "$tmp/x07.json" --report-json >"$tmp/provides.json"

python3 - "$tmp/provides.json" <<'PY'
import json, sys

r = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert r.get("schema_version") == "x07.pkg.provides.report@0.1.0", r.get("schema_version")
assert r.get("ok") is True, r
providers = r.get("providers", [])
assert isinstance(providers, list) and len(providers) >= 1, providers
print("ok: pkg provides returns providers")
PY

set +e
"$X07_BIN" pkg provides ext.checksum.crc32c --project "$tmp/x07.json" --report-json >"$tmp/provides_ext.json" 2>"$tmp/provides_ext.err"
rc=$?
set -e
if [[ $rc -ne 0 ]]; then
  echo "ERROR: expected 'x07 pkg provides ext.checksum.crc32c' to succeed (rc=$rc)" >&2
  cat "$tmp/provides_ext.err" >&2 || true
  exit 1
fi

python3 - "$tmp/provides_ext.json" <<'PY'
import json, sys

r = json.load(open(sys.argv[1], "r", encoding="utf-8"))
providers = r.get("providers") or []
has = any(isinstance(p, dict) and p.get("kind") == "catalog" and p.get("name") == "ext-checksum-rs" for p in providers)
assert has, providers
print("ok: pkg provides finds catalog providers")
PY

echo "ok: check_pkg_provides_smoke"
