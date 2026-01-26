#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

./scripts/ci/check_tools.sh >/dev/null

X07_BIN="${X07_BIN:-$(./scripts/ci/find_x07.sh)}"

./scripts/ci/ensure_runners.sh

# Ensure helper binaries exist (x07-proc-echo used by process spawn).
./scripts/build_os_helpers.sh

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/src"
mkdir -p "$tmp/deps/x07"

# `x07 run --project` runs with the project root as the OS-world CWD.
# Copy the helper binaries into the temp project so relative exec paths resolve.
for name in x07-proc-echo x07-proc-worker-frame-echo; do
  if [[ -f "$ROOT/deps/x07/$name" ]]; then
    cp -f "$ROOT/deps/x07/$name" "$tmp/deps/x07/$name"
    chmod 0755 "$tmp/deps/x07/$name"
  fi

  if [[ -f "$ROOT/deps/x07/$name.exe" ]]; then
    cp -f "$ROOT/deps/x07/$name.exe" "$tmp/deps/x07/$name.exe"
    chmod 0755 "$tmp/deps/x07/$name.exe"
  fi
done

cat >"$tmp/x07.json" <<'JSON'
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "src/main.x07.json",
  "module_roots": ["src"],
  "profiles": {
    "os": { "world": "run-os" },
    "sandbox": { "world": "run-os-sandboxed", "policy": "policy-sandbox.json" }
  },
  "default_profile": "os"
}
JSON

cat >"$tmp/x07.lock.json" <<'JSON'
{"schema_version":"x07.lock@0.2.0","dependencies":[]}
JSON

"$X07_BIN" policy init \
  --template worker-parallel \
  --project "$tmp/x07.json" \
  --out "$tmp/policy-sandbox.json" \
  --force \
  --emit report \
  >"$tmp/policy.init.report.json"

cat >"$tmp/src/main.x07.json" <<'JSON'
{
  "schema_version": "x07.x07ast@0.2.0",
  "kind": "entry",
  "module_id": "main",
  "imports": [
    "std.bytes",
    "std.os.process",
    "std.os.process.caps_v1",
    "std.os.process.req_v1"
  ],
  "decls": [
    {
      "kind": "defasync",
      "name": "main.worker",
      "params": [],
      "result": "bytes",
      "body": [
        "begin",
        ["let", "exe", ["bytes.lit", "deps/x07/x07-proc-echo"]],
        ["let", "reqb", ["std.os.process.req_v1.new", "exe"]],
        ["let", "req", ["std.os.process.req_v1.finish", "reqb"]],
        ["let", "caps", ["std.os.process.caps_v1.pack", 1048576, 1048576, 1000, 0]],
        ["let", "h", ["std.os.process.spawn_piped_v1", "req", "caps"]],
        ["let", "chunk", ["bytes.lit", "hello"]],
        ["std.os.process.stdin_write_v1", "h", "chunk"],
        ["std.os.process.stdin_close_v1", "h"],
        ["os.process.join_exit_v1", "h"],
        ["let", "exit_code", ["std.os.process.take_exit_v1", "h"]],
        ["let", "stdout", ["std.os.process.stdout_read_v1", "h", 4096]],
        ["let", "stderr", ["std.os.process.stderr_read_v1", "h", 4096]],
        ["std.os.process.drop_v1", "h"],
        ["let", "want", ["bytes.lit", "hello"]],
        [
          "if",
          [
            "&",
            ["=", "exit_code", 0],
            [
              "&",
              ["std.bytes.eq", ["bytes.view", "stdout"], ["bytes.view", "want"]],
              ["=", ["std.bytes.len", "stderr"], 0]
            ]
          ],
          ["bytes.lit", "ok"],
          ["bytes.lit", "bad"]
        ]
      ]
    }
  ],
  "solve": ["begin", ["let", "t", ["main.worker"]], ["task.spawn", "t"], ["await", "t"]]
}
JSON

for profile in os sandbox; do
  "$X07_BIN" run --project "$tmp/x07.json" --profile "$profile" --report wrapped --report-out "$tmp/run_${profile}.json"

  python3 - "$tmp/run_${profile}.json" "$profile" <<'PY'
import base64, json, sys

path = sys.argv[1]
profile = sys.argv[2]

r = json.load(open(path, "r", encoding="utf-8"))
assert r.get("schema_version") == "x07.run.report@0.1.0", r.get("schema_version")
assert r.get("runner") == "os", r.get("runner")
rep = r.get("report") or {}
assert rep.get("exit_code") == 0, rep.get("exit_code")
compile = rep.get("compile") or {}
assert compile.get("ok") is True, compile
assert compile.get("exit_status") == 0, compile.get("exit_status")
solve = rep.get("solve") or {}
assert solve.get("ok") is True, solve
assert solve.get("exit_status") == 0, solve.get("exit_status")
out = base64.b64decode(solve.get("solve_output_b64") or "")
assert out == b"ok", out
print(f"ok: concurrency+parallelism ({profile})")
PY
done

echo "ok: check_concurrency_parallelism_smoke"
