#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

step() {
  echo
  echo "==> $*"
}

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: missing tool: $1" >&2; exit 2; }
}

pick_python() {
  if [[ -n "${X07_PYTHON:-}" ]]; then
    echo "$X07_PYTHON"
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    echo "python3"
    return
  fi
  echo "python"
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)
      case "$arch" in
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) echo "unknown" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64) echo "x86_64-apple-darwin" ;;
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        *) echo "unknown" ;;
      esac
      ;;
    *)
      echo "unknown"
      ;;
  esac
}

detect_platform_label() {
  case "$(uname -s)" in
    Linux) echo "Linux" ;;
    Darwin) echo "macOS" ;;
    *) echo "unknown" ;;
  esac
}

root="$(repo_root)"
cd "$root"

need bash
need tar
need cargo
need curl

python_bin="$(pick_python)"
need "$python_bin"

target="$(detect_target)"
platform="$(detect_platform_label)"
if [[ "$target" == "unknown" || "$platform" == "unknown" ]]; then
  echo "ERROR: unsupported host for installer smoke: target=$target platform=$platform" >&2
  exit 2
fi

tmp="$(mktemp -d)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

mode="${X07_INSTALL_SMOKE_MODE:-local}"

install_root="$tmp/x07root"
mkdir -p "$install_root"

channels_url=""
installer_path=""

if [[ "$mode" == "local" ]]; then
  step "build release binaries (including x07up)"
  cargo build --release -p x07 -p x07c -p x07-host-runner -p x07-os-runner -p x07import-cli -p x07up

  tag="v0.0.0-ci"
  artifacts="$tmp/artifacts"
  mkdir -p "$artifacts"

  step "package x07up archive"
  mkdir -p "$tmp/pkg/x07up"
  cp -f "target/release/x07up" "$tmp/pkg/x07up/x07up"
  chmod 0755 "$tmp/pkg/x07up/x07up"
  x07up_archive="$artifacts/x07up-${tag}-${target}.tar.gz"
  tar -czf "$x07up_archive" -C "$tmp/pkg/x07up" x07up

  step "package toolchain archive (CI minimal)"
  toolchain_archive="$artifacts/x07-${tag}-${target}.tar.gz"
  ./scripts/build_toolchain_tarball.sh --tag "$tag" --platform "$platform" --out "$toolchain_archive" --skip-native-backends

  step "start local artifacts server"
  server_json="$tmp/server.json"
  "$python_bin" scripts/ci/local_http_server.py --root "$artifacts" --ready-json "$server_json" --quiet &
  server_pid="$!"
  trap 'kill "$server_pid" >/dev/null 2>&1 || true' EXIT

  for _ in $(seq 1 50); do
    [[ -f "$server_json" ]] && break
    sleep 0.05
  done
  if [[ ! -f "$server_json" ]]; then
    echo "ERROR: local server did not publish ready json" >&2
    exit 2
  fi
  base_url="$("$python_bin" - "$server_json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
url = doc.get("url", "")
print(url.rstrip("/"))
PY
)"

  step "write local channels.json"
  "$python_bin" scripts/ci/make_channels_json.py \
    --base-url "$base_url" \
    --out "$artifacts/channels.json" \
    --tag "$tag" \
    --target "$target" \
    --toolchain-file "$toolchain_archive" \
    --x07up-file "$x07up_archive"

  channels_url="$base_url/channels.json"
  installer_path="$root/dist/install/install.sh"
else
  channels_url="${X07_CHANNELS_URL:-https://x07lang.org/install/channels.json}"
  installer_path="${X07_INSTALLER_SH:-https://x07lang.org/install.sh}"
fi

step "run installer (mode=$mode)"
install_report="$tmp/install.report.json"

if [[ "$installer_path" == http* ]]; then
  curl -fsSL "$installer_path" | bash -s -- \
    --yes \
    --root "$install_root" \
    --channel stable \
    --channels-url "$channels_url" \
    --no-modify-path \
    --json \
    >"$install_report"
else
  if [[ ! -x "$installer_path" ]]; then
    echo "ERROR: installer not executable: $installer_path" >&2
    exit 2
  fi
  bash "$installer_path" \
    --yes \
    --root "$install_root" \
    --channel stable \
    --channels-url "$channels_url" \
    --no-modify-path \
    --json \
    >"$install_report"
fi

export PATH="$install_root/bin:$PATH"

step "smoke: x07up show"
x07up show --json >"$tmp/x07up.show.json"
"$python_bin" - "$tmp/x07up.show.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
for k in ("schema_version","toolchains","active","channels"):
    if k not in doc:
        raise SystemExit(f"ERROR: x07up show missing key: {k}")
print("ok: x07up show shape")
PY

step "smoke: x07 help"
x07 --help >/dev/null

step "smoke: init+run (host profile)"
proj="$tmp/proj"
mkdir -p "$proj"
cd "$proj"
x07 --init >/dev/null

printf "hello" > input.bin
x07 run --profile test --input input.bin --report wrapped --report-out .x07/run.host.json >/dev/null

"$python_bin" - ".x07/run.host.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if doc.get("schema_version") != "x07.run.report@0.1.0":
    raise SystemExit("ERROR: wrapped report schema_version mismatch")
if doc.get("runner") != "host":
    raise SystemExit("ERROR: expected runner=host")
rep = doc.get("report") or {}
if rep.get("exit_code") not in (0, "0"):
    sys.stderr.write(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    raise SystemExit("ERROR: host run exit_code != 0")
compile_ok = (rep.get("compile") or {}).get("ok")
solve_ok = (rep.get("solve") or {}).get("ok")
if compile_ok is not True or solve_ok is not True:
    sys.stderr.write(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    raise SystemExit("ERROR: host run compile/solve not ok")
roots = doc.get("target", {}).get("resolved_module_roots") or []
if not any(r.replace("\\","/").endswith("/src") or r == "src" for r in roots):
    raise SystemExit("ERROR: expected src in resolved_module_roots")
print("ok: host wrapped report ok")
PY

step "smoke: test harness baseline (stdlib.lock fallback)"
x07 test --manifest tests/tests.json >"$tmp/x07test.report.json"
"$python_bin" - "$tmp/x07test.report.json" <<'PY'
import json, sys
from pathlib import Path
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if doc.get("schema_version") != "x07.x07test@0.2.0":
    raise SystemExit("ERROR: x07test schema_version mismatch")
summary = doc.get("summary") or {}
if summary.get("passed") != 1:
    raise SystemExit(f"ERROR: expected 1 passed test, got: {summary.get('passed')}")
stdlib_lock = (doc.get("invocation") or {}).get("stdlib_lock")
if not isinstance(stdlib_lock, str) or not stdlib_lock.endswith("stdlib.lock"):
    raise SystemExit("ERROR: missing invocation.stdlib_lock")
if not Path(stdlib_lock).is_file():
    raise SystemExit(f"ERROR: invocation.stdlib_lock does not exist: {stdlib_lock}")
print("ok: x07 test ok")
PY

step "smoke: run (os profile) if compiler is present"
if command -v cc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1; then
  x07 run --profile os --input input.bin --report wrapped --report-out .x07/run.os.json >/dev/null
  "$python_bin" - ".x07/run.os.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if doc.get("schema_version") != "x07.run.report@0.1.0":
    raise SystemExit("ERROR: wrapped report schema_version mismatch")
if doc.get("runner") != "os":
    raise SystemExit("ERROR: expected runner=os")
rep = doc.get("report") or {}
if rep.get("exit_code") not in (0, "0"):
    raise SystemExit("ERROR: os run exit_code != 0")
compile_ok = (rep.get("compile") or {}).get("ok")
solve_ok = (rep.get("solve") or {}).get("ok")
if compile_ok is not True or solve_ok is not True:
    raise SystemExit("ERROR: os run compile/solve not ok")
roots = doc.get("target", {}).get("resolved_module_roots") or []
if not any(r.replace("\\","/").endswith("stdlib/os/0.2.0/modules") for r in roots):
    raise SystemExit("ERROR: expected stdlib/os module root for os runner")
print("ok: os wrapped report ok")
PY
else
  echo "warn: no C compiler detected; skipping os profile smoke"
fi

step "smoke: agent init produces AGENT.md"
x07up agent init --project "$proj" --with-skills project >/dev/null
test -f "$proj/AGENT.md"

echo
echo "ok: installer smoke passed"
