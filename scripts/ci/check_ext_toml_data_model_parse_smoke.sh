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

dm_ver="$(x07_ext_pkg_latest_version x07-ext-data-model)"
toml_ver="$(x07_ext_pkg_latest_version x07-ext-toml-rs)"
json_ver="$(x07_ext_pkg_latest_version x07-ext-json-rs)"
yaml_ver="$(x07_ext_pkg_latest_version x07-ext-yaml-rs)"
unicode_ver="$(x07_ext_pkg_latest_version x07-ext-unicode-rs)"

mkdir -p "$tmp/src" "$tmp/.x07/deps"

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
  "imports": ["ext.data_model", "ext.toml.data_model"],
  "decls": [],
  "solve": [
    "begin",
    ["let", "ok", ["bytes.lit", "ok"]],
    ["let", "bad", ["bytes.lit", "bad"]],
    ["let", "src", ["bytes.lit", "foo = 1\nbar = \"hi\"\n"]],
    ["let", "doc", ["ext.toml.data_model.parse", ["bytes.view", "src"]]],
    ["let", "docv", ["bytes.view", "doc"]],
    ["if", ["!=", ["ext.data_model.doc_is_err", "docv"], 0], ["return", "bad"], 0],
    ["let", "root_off", ["ext.data_model.root_offset", "docv"]],
    ["if", ["<", "root_off", 0], ["return", "bad"], 0],
    ["if", ["!=", ["ext.data_model.kind_at", "docv", "root_off"], 5], ["return", "bad"], 0],
    ["let", "k_foo", ["bytes.lit", "foo"]],
    ["let", "k_bar", ["bytes.lit", "bar"]],
    ["let", "off_foo", ["ext.data_model.map_find", "docv", "root_off", ["bytes.view", "k_foo"]]],
    ["let", "off_bar", ["ext.data_model.map_find", "docv", "root_off", ["bytes.view", "k_bar"]]],
    ["if", ["<", "off_foo", 0], ["return", "bad"], 0],
    ["if", ["<", "off_bar", 0], ["return", "bad"], 0],
    ["if", ["!=", ["ext.data_model.kind_at", "docv", "off_foo"], 2], ["return", "bad"], 0],
    ["let", "foo_s", ["ext.data_model.number_get", "docv", "off_foo"]],
    ["let", "exp_one", ["bytes.lit", "1"]],
    [
      "if",
      [
        "!=",
        ["bytes.cmp_range", "foo_s", 0, ["bytes.len", "foo_s"], "exp_one", 0, ["bytes.len", "exp_one"]],
        0
      ],
      ["return", "bad"],
      0
    ],
    ["if", ["!=", ["ext.data_model.kind_at", "docv", "off_bar"], 3], ["return", "bad"], 0],
    ["let", "bar_s", ["ext.data_model.string_get", "docv", "off_bar"]],
    ["let", "exp_hi", ["bytes.lit", "hi"]],
    [
      "if",
      [
        "!=",
        ["bytes.cmp_range", "bar_s", 0, ["bytes.len", "bar_s"], "exp_hi", 0, ["bytes.len", "exp_hi"]],
        0
      ],
      ["return", "bad"],
      0
    ],
    "ok"
  ]
}
JSON

"$X07_BIN" pkg lock --offline --project "$tmp/x07.json" >/dev/null

"$X07_BIN" run --project "$tmp/x07.json" --report wrapped --report-out "$tmp/run_report.json" >/dev/null

"$python_bin" - "$tmp/run_report.json" <<'PY'
import base64
import json
import sys

r = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert r.get("schema_version") == "x07.run.report@0.1.0", r.get("schema_version")
rep = r.get("report") or {}
assert (rep.get("compile") or {}).get("ok") is True, rep.get("compile")
assert (rep.get("solve") or {}).get("ok") is True, rep.get("solve")
out = base64.b64decode((rep.get("solve") or {}).get("solve_output_b64") or "")
assert out == b"ok", out
print("ok: ext.toml.data_model.parse")
PY

echo "ok: check_ext_toml_data_model_parse_smoke"

