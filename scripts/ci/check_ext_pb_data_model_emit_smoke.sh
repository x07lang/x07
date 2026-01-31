#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null
./scripts/ci/ensure_runners.sh

source ./scripts/ci/lib_ext_packages.sh

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

X07_BIN="${X07_BIN:-$(./scripts/ci/find_x07.sh)}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pb_ver="$(x07_ext_pkg_latest_version x07-ext-pb-rs)"
dm_ver="$(x07_ext_pkg_latest_version x07-ext-data-model)"
toml_ver="$(x07_ext_pkg_latest_version x07-ext-toml-rs)"
json_ver="$(x07_ext_pkg_latest_version x07-ext-json-rs)"
yaml_ver="$(x07_ext_pkg_latest_version x07-ext-yaml-rs)"
unicode_ver="$(x07_ext_pkg_latest_version x07-ext-unicode-rs)"

mkdir -p "$tmp/src" "$tmp/tests" "$tmp/.x07/deps"

rm -rf "$tmp/.x07/deps/ext-pb-rs/$pb_ver"
mkdir -p "$tmp/.x07/deps/ext-pb-rs"
cp -R "$root/packages/ext/x07-ext-pb-rs/$pb_ver" "$tmp/.x07/deps/ext-pb-rs/$pb_ver"

rm -rf "$tmp/.x07/deps/ext-data-model/$dm_ver"
mkdir -p "$tmp/.x07/deps/ext-data-model"
cp -R "$root/packages/ext/x07-ext-data-model/$dm_ver" "$tmp/.x07/deps/ext-data-model/$dm_ver"

rm -rf "$tmp/.x07/deps/ext-toml-rs/$toml_ver"
mkdir -p "$tmp/.x07/deps/ext-toml-rs"
cp -R "$root/packages/ext/x07-ext-toml-rs/$toml_ver" "$tmp/.x07/deps/ext-toml-rs/$toml_ver"

rm -rf "$tmp/.x07/deps/ext-json-rs/$json_ver"
mkdir -p "$tmp/.x07/deps/ext-json-rs"
cp -R "$root/packages/ext/x07-ext-json-rs/$json_ver" "$tmp/.x07/deps/ext-json-rs/$json_ver"

rm -rf "$tmp/.x07/deps/ext-yaml-rs/$yaml_ver"
mkdir -p "$tmp/.x07/deps/ext-yaml-rs"
cp -R "$root/packages/ext/x07-ext-yaml-rs/$yaml_ver" "$tmp/.x07/deps/ext-yaml-rs/$yaml_ver"

rm -rf "$tmp/.x07/deps/ext-unicode-rs/$unicode_ver"
mkdir -p "$tmp/.x07/deps/ext-unicode-rs"
cp -R "$root/packages/ext/x07-ext-unicode-rs/$unicode_ver" "$tmp/.x07/deps/ext-unicode-rs/$unicode_ver"

cat >"$tmp/x07.json" <<JSON
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "src/main.x07.json",
  "module_roots": ["src"],
  "dependencies": [
    {"name": "ext-pb-rs", "version": "$pb_ver", "path": ".x07/deps/ext-pb-rs/$pb_ver"},
    {"name": "ext-data-model", "version": "$dm_ver", "path": ".x07/deps/ext-data-model/$dm_ver"},
    {"name": "ext-toml-rs", "version": "$toml_ver", "path": ".x07/deps/ext-toml-rs/$toml_ver"},
    {"name": "ext-json-rs", "version": "$json_ver", "path": ".x07/deps/ext-json-rs/$json_ver"},
    {"name": "ext-yaml-rs", "version": "$yaml_ver", "path": ".x07/deps/ext-yaml-rs/$yaml_ver"},
    {"name": "ext-unicode-rs", "version": "$unicode_ver", "path": ".x07/deps/ext-unicode-rs/$unicode_ver"}
  ],
  "lockfile": "x07.lock.json"
}
JSON

cat >"$tmp/x07.lock.json" <<'JSON'
{"schema_version":"x07.lock@0.2.0","dependencies":[]}
JSON

cat >"$tmp/src/main.x07.json" <<'JSON'
{
  "schema_version": "x07.x07ast@0.3.0",
  "kind": "entry",
  "module_id": "main",
  "imports": [],
  "decls": [],
  "solve": ["bytes.lit", "noop"]
}
JSON

cat >"$tmp/tests/tests.json" <<'JSON'
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": [
    {
      "id": "pb_emit_v1_roundtrip",
      "world": "solve-pure",
      "entry": "ext.pb.tests.test_data_model_emit_v1_roundtrip",
      "expect": "pass"
    },
    {
      "id": "pb_emit_schema_v2_roundtrip",
      "world": "solve-pure",
      "entry": "ext.pb.tests.test_data_model_emit_schema_v2_roundtrip",
      "expect": "pass"
    },
    {
      "id": "pb_emit_schema_v2_err_message_not_found",
      "world": "solve-pure",
      "entry": "ext.pb.tests.test_data_model_emit_schema_v2_err_message_not_found",
      "expect": "pass"
    }
  ]
}
JSON

"$X07_BIN" pkg lock --offline --project "$tmp/x07.json" >/dev/null

"$X07_BIN" test --manifest "$tmp/tests/tests.json" --no-fail-fast --json=false

echo "ok: check_ext_pb_data_model_emit_smoke"

