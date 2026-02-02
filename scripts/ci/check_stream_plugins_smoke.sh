#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

./scripts/ci/check_tools.sh >/dev/null

X07_BIN="${X07_BIN:-$(./scripts/ci/find_x07.sh)}"

./scripts/ci/ensure_runners.sh
./scripts/ci/ensure_stream_xf_backend.sh

tmp="$ROOT/tmp/stream_plugins_smoke_$$"
rm -rf "$tmp"
mkdir -p "$tmp/src"
trap 'rm -rf "$tmp"' EXIT

cat >"$tmp/x07.json" <<'JSON'
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "src/main.x07.json",
  "module_roots": ["src"]
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
  "solve": ["begin",
    ["let","in",["bytes.lit","hi"]],
    ["std.stream.pipe_v1",
      ["std.stream.cfg_v1",
        ["chunk_max_bytes", 64],
        ["bufread_cap_bytes", 64],
        ["max_in_bytes", 64],
        ["max_out_bytes", 64],
        ["max_items", 10]
      ],
      ["std.stream.src.bytes_v1", ["std.stream.expr_v1", "in"]],
      ["std.stream.chain_v1",
        ["std.stream.xf.plugin_v1",
          ["id", ["bytes.lit", "xf.frame_u32le_v1"]],
          ["cfg", ["std.stream.expr_v1", ["bytes.lit", ""]]]
        ]
      ],
      ["std.stream.sink.collect_bytes_v1"]
    ]
  ]
}
JSON

"$X07_BIN" run --project "$tmp/x07.json" --report wrapped --report-out "$tmp/run_report_1.json"
"$X07_BIN" run --project "$tmp/x07.json" --report wrapped --report-out "$tmp/run_report_2.json"

python3 - "$tmp/run_report_1.json" "$tmp/run_report_2.json" <<'PY'
import base64, json, struct, sys


def payload_from_run_report(path: str) -> bytes:
    r = json.load(open(path, "r", encoding="utf-8"))
    assert r.get("schema_version") == "x07.run.report@0.1.0", r.get("schema_version")
    assert r.get("runner") == "host", r.get("runner")
    rep = r.get("report") or {}
    assert rep.get("schema_version") == "x07-host-runner.report@0.3.0", rep.get("schema_version")
    assert rep.get("exit_code") == 0, rep.get("exit_code")

    compile = rep.get("compile") or {}
    assert compile.get("ok") is True, compile

    native_requires = compile.get("native_requires") or {}
    reqs = native_requires.get("requires") or []
    assert any(
        (req.get("backend_id") == "x07.stream.xf" and req.get("abi_major") == 1)
        for req in reqs
    ), reqs

    solve = rep.get("solve") or {}
    assert solve.get("ok") is True, solve
    assert solve.get("exit_status") == 0, solve.get("exit_status")
    out = base64.b64decode(solve.get("solve_output_b64") or "")

    assert out[:1] == b"\x01", out[:1]
    payload_len = struct.unpack_from("<I", out, 17)[0]
    payload = out[21 : 21 + payload_len]
    assert len(payload) == payload_len, (len(payload), payload_len)
    return payload


p1 = payload_from_run_report(sys.argv[1])
p2 = payload_from_run_report(sys.argv[2])
assert p1 == p2, (p1, p2)

exp = struct.pack("<I", 2) + b"hi"
assert p1 == exp, (p1, exp)
print("ok: stream plugin smoke")
PY

echo "ok: check_stream_plugins_smoke"

