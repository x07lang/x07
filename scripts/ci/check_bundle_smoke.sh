#!/usr/bin/env bash
set -euo pipefail

# CI gate: `x07 bundle` produces a native executable that can run with no x07 toolchain installed.
# This gate uses a minimal fixture that echoes its input bytes. The wrapper must encode argv as argv_v1.

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

export X07_SANDBOX_BACKEND="${X07_SANDBOX_BACKEND:-os}"
export X07_I_ACCEPT_WEAKER_ISOLATION="${X07_I_ACCEPT_WEAKER_ISOLATION:-1}"

./scripts/ci/check_tools.sh >/dev/null

step() {
  echo
  echo "==> $*"
}

pick_python() {
  if [[ -n "${X07_PYTHON:-}" ]]; then
    echo "$X07_PYTHON"
    return
  fi
  if [[ -x ".venv/bin/python" ]]; then
    echo ".venv/bin/python"
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    echo "python3"
    return
  fi
  echo "python"
}

is_windows() {
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) return 0 ;;
    *) return 1 ;;
  esac
}

is_linux() {
  [[ "$(uname -s)" == "Linux" ]]
}

python_bin="$(pick_python)"

# Resolve relative python paths (e.g. .venv/bin/python) to be stable across `cd`.
if [[ "${python_bin}" == */* && "${python_bin}" != /* ]]; then
  python_bin="$root/$python_bin"
fi

docker_image="${X07_BUNDLE_SMOKE_DOCKER_IMAGE:-}"
if [[ -z "$docker_image" ]] && is_linux && [[ -f "/etc/os-release" ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  if [[ "${ID:-}" == "ubuntu" && "${VERSION_ID:-}" == "24.04" ]]; then
    docker_image="ubuntu:24.04"
  else
    docker_image="debian:bookworm-slim"
  fi
fi
if [[ -z "$docker_image" ]]; then
  docker_image="debian:bookworm-slim"
fi

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "$x07_bin" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

if [[ ! -x "$x07_bin" && ! -f "$x07_bin" ]]; then
  echo "ERROR: x07 binary not found/executable at: $x07_bin" >&2
  exit 2
fi

# Temp workdir (Windows/MSYS2: keep under repo to avoid path issues).
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    mkdir -p "$root/tmp"
    tmp_dir="$(mktemp -d -p "$root/tmp" x07_bundle_smoke_XXXXXX)"
    ;;
  *)
    tmp_dir="$(mktemp -t x07_bundle_smoke_XXXXXX -d)"
    ;;
esac

keep_tmp="${X07_BUNDLE_SMOKE_KEEP_TMP:-}"
cleanup() {
  if [[ -z "$keep_tmp" ]]; then
    rm -rf "$tmp_dir" || true
  else
    echo "[bundle-smoke] kept tmp dir: $tmp_dir"
  fi
}
trap cleanup EXIT

exe_ext=""
if is_windows; then
  exe_ext=".exe"
fi

run_and_capture_stdout_bin() {
  local outdir="$1"
  local bin_name="$2"
  local stdout_bin="$3"
  local stderr_txt="$4"

  rm -f "$stdout_bin" "$stderr_txt" || true

  if is_linux && [[ "${X07_BUNDLE_SMOKE_DOCKER:-1}" != "0" ]]; then
    if ! command -v docker >/dev/null 2>&1; then
      if [[ "${CI:-}" == "true" ]] && [[ ! -f "/.dockerenv" ]]; then
        echo "ERROR: docker missing on Linux CI host (required for no-toolchain bundle gate)" >&2
        exit 2
      fi
      echo "[bundle-smoke] WARN: docker not found; falling back to local execution"
    else
      # Strong guarantee: run inside a minimal container (no x07 installed).
      if ! docker run --rm \
        -v "$outdir:/work:ro" \
        -w /work \
        "$docker_image" \
        "./$bin_name" --alpha A --beta B \
        >"$stdout_bin" 2>"$stderr_txt"; then
        echo "ERROR: bundle-smoke docker run failed (image=$docker_image, bin=$bin_name)" >&2
        if [[ -f "$stderr_txt" ]]; then
          echo "stderr:" >&2
          cat "$stderr_txt" >&2 || true
        fi
        return 1
      fi
      return 0
    fi
  fi

  # Fallback: execute locally from an isolated directory.
  local run_dir="$outdir/run"
  mkdir -p "$run_dir"
  cp -f "$outdir/$bin_name" "$run_dir/$bin_name"
  if [[ -d "$outdir/deps" ]]; then
    cp -R "$outdir/deps" "$run_dir/deps"
  fi

  if is_windows; then
    (cd "$run_dir" && "./$bin_name" --alpha A --beta B >"$stdout_bin") 2>"$stderr_txt"
  else
    (cd "$run_dir" && env -i PATH="/usr/bin:/bin" "./$bin_name" --alpha A --beta B >"$stdout_bin") 2>"$stderr_txt"
  fi
}

assert_argv_v1() {
  local stdout_bin="$1"
  local profile="$2"

  "$python_bin" - "$stdout_bin" "$profile" <<'PY'
import struct, sys
from pathlib import Path

path = Path(sys.argv[1])
profile = sys.argv[2]

data = path.read_bytes()
if len(data) < 4:
    raise SystemExit(f"[{profile}] stdout too short for argv_v1: {len(data)} bytes")

argc = struct.unpack_from("<I", data, 0)[0]
off = 4
args = []
for i in range(argc):
    if off + 4 > len(data):
        raise SystemExit(f"[{profile}] truncated argv_v1 at arg {i}: missing len")
    n = struct.unpack_from("<I", data, off)[0]
    off += 4
    if off + n > len(data):
        raise SystemExit(f"[{profile}] truncated argv_v1 at arg {i}: need {n} bytes")
    args.append(data[off:off+n])
    off += n

if off != len(data):
    raise SystemExit(f"[{profile}] argv_v1 has trailing bytes: parsed={off} total={len(data)}")

expected = [b"echo-argv", b"--alpha", b"A", b"--beta", b"B"]
if args != expected:
    raise SystemExit(f"[{profile}] argv_v1 mismatch:\n  got: {args!r}\n  exp: {expected!r}")

print(f"ok: argv_v1 ({profile})")
PY
}

assert_stdout_ok() {
  local stdout_bin="$1"
  local fixture="$2"
  local profile="$3"

  "$python_bin" - "$stdout_bin" "$fixture" "$profile" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
fixture = sys.argv[2]
profile = sys.argv[3]

data = path.read_bytes()
if data != b"ok":
    raise SystemExit(f"[{fixture}::{profile}] expected stdout b'ok', got: {data!r}")
print(f"ok: stdout 'ok' ({fixture}::{profile})")
PY
}

./scripts/build_os_helpers.sh >/dev/null

fixtures=(echo-argv async-only process-async-join)
profiles=(test os sandbox)

for fixture_name in "${fixtures[@]}"; do
  fixture_src="$root/ci/fixtures/bundle/$fixture_name"
  if [[ ! -d "$fixture_src" ]]; then
    echo "ERROR: missing bundle fixture dir: $fixture_src" >&2
    exit 2
  fi

  fixture="$tmp_dir/$fixture_name"
  cp -R "$fixture_src" "$fixture"

  cd "$fixture"

  template=""
  policy_path=""
  case "$fixture_name" in
    echo-argv)
      template="cli"
      policy_path="$fixture/.x07/policies/base/cli.sandbox.base.policy.json"
      ;;
    async-only)
      template="worker"
      policy_path="$fixture/.x07/policies/base/worker.sandbox.base.policy.json"
      ;;
    process-async-join)
      template="worker-parallel"
      policy_path="$fixture/.x07/policies/base/worker-parallel.sandbox.base.policy.json"
      ;;
    *)
      echo "ERROR: unknown fixture: $fixture_name" >&2
      exit 2
      ;;
  esac

  step "policy init (sandbox base policy): fixture=$fixture_name template=$template"
  "$x07_bin" policy init --template "$template" --project x07.json --emit report >"$tmp_dir/${fixture_name}.policy.init.report.json"

  if [[ ! -f "$policy_path" ]]; then
    echo "ERROR: policy init did not create expected file: $policy_path" >&2
    exit 2
  fi

  for profile in "${profiles[@]}"; do
    step "bundle: fixture=$fixture_name profile=$profile"

    outdir="$tmp_dir/out/$fixture_name/$profile"
    mkdir -p "$outdir"

    bin_name="${fixture_name}${exe_ext}"
    out_bin="$outdir/$bin_name"

    emit_dir="$outdir/emit"
    mkdir -p "$emit_dir"

    report="$outdir/bundle.report.json"

    # `x07 bundle` must print x07.bundle.report@0.2.0 JSON to stdout (machine-clean).
    "$x07_bin" bundle \
      --project x07.json \
      --profile "$profile" \
      --out "$out_bin" \
      --emit-dir "$emit_dir" \
      >"$report"

    if [[ ! -f "$out_bin" ]]; then
      echo "ERROR: bundle did not produce expected binary: $out_bin" >&2
      echo "report:" >&2
      sed -n '1,120p' "$report" >&2 || true
      exit 2
    fi

    if ! is_windows; then
      chmod 0755 "$out_bin" >/dev/null 2>&1 || true
    fi

    if [[ "$fixture_name" == "process-async-join" ]]; then
      mkdir -p "$outdir/deps/x07"
      cp -f "$root/deps/x07/x07-proc-echo" "$outdir/deps/x07/x07-proc-echo"
      if [[ -f "$root/deps/x07/x07-proc-echo.exe" ]]; then
        cp -f "$root/deps/x07/x07-proc-echo.exe" "$outdir/deps/x07/x07-proc-echo.exe"
      fi
      chmod 0755 "$outdir/deps/x07/x07-proc-echo" >/dev/null 2>&1 || true
    fi

    stdout_bin="$outdir/stdout.bin"
    stderr_txt="$outdir/stderr.txt"

    step "run bundled binary (no toolchain): fixture=$fixture_name profile=$profile"
    run_and_capture_stdout_bin "$outdir" "$bin_name" "$stdout_bin" "$stderr_txt"

    if [[ ! -f "$stdout_bin" ]]; then
      echo "ERROR: missing stdout capture: $stdout_bin" >&2
      [[ -f "$stderr_txt" ]] && { echo "stderr:" >&2; cat "$stderr_txt" >&2; }
      exit 2
    fi

    step "validate output: fixture=$fixture_name profile=$profile"
    if [[ "$fixture_name" == "echo-argv" ]]; then
      assert_argv_v1 "$stdout_bin" "$profile"
    else
      assert_stdout_ok "$stdout_bin" "$fixture_name" "$profile"
    fi
  done
done

echo
echo "ok: bundle smoke passed"
