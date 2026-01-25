#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

source ./scripts/ci/lib_ext_packages.sh

mkdir -p tmp tmp/tmp

cargo build -p x07-os-runner >/dev/null

test_modules="tests/external_os/modules"

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  elif command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  else
    python_bin="python"
  fi
fi

ffi_lib_args() {
  local package_manifest="$1"
  "$python_bin" - "$package_manifest" <<'PY'
import json
import shutil
import subprocess
import sys

path = sys.argv[1]
doc = json.load(open(path, "r", encoding="utf-8"))
meta = doc.get("meta") or {}
libs = meta.get("ffi_libs") or []
out = []

if sys.platform.startswith("win") and doc.get("name") == "ext-sockets-c":
    out.append("-lws2_32")

if sys.platform.startswith("win") and doc.get("name") == "ext-curl-c":
    pkg_config = shutil.which("pkg-config") or shutil.which("pkgconf")
    if pkg_config:
        r = subprocess.run(
            [pkg_config, "--libs", "--static", "libcurl"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        flags = (r.stdout or "").strip()
        if r.returncode == 0 and flags:
            for tok in flags.split():
                if not tok.startswith("-l"):
                    continue
                if tok == "-lssl":
                    tok = "-l:libssl.dll.a"
                elif tok == "-lcrypto":
                    tok = "-l:libcrypto.dll.a"
                out.append(tok)
            libs = [lib for lib in libs if lib != "curl"]

openssl_requested = any(lib in ("ssl", "crypto") for lib in libs) or any(
    tok in ("-lssl", "-lcrypto", "-l:libssl.dll.a", "-l:libcrypto.dll.a") for tok in out
)

if any(lib in ("ssl", "crypto") for lib in libs):
    brew = shutil.which("brew")
    if brew:
        for formula in ("openssl@3", "openssl@1.1", "openssl"):
            try:
                r = subprocess.run(
                    [brew, "--prefix", formula],
                    check=False,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                    text=True,
                )
            except OSError:
                break
            prefix = (r.stdout or "").strip()
            if r.returncode == 0 and prefix:
                incdir = f"{prefix}/include"
                libdir = f"{prefix}/lib"
                out.append(f"-I{incdir}")
                out.append(f"-L{libdir}")
                out.append(f"-Wl,-rpath,{libdir}")
                break

for lib in libs:
    if sys.platform.startswith("win") and lib in ("ssl", "crypto"):
        out.append(f"-l:lib{lib}.dll.a")
    else:
        out.append(f"-l{lib}")

if sys.platform.startswith("win") and openssl_requested:
    # OpenSSL on Windows typically needs additional system libs for crypto + sockets.
    # Keep these after -lssl/-lcrypto so the linker can resolve dependencies.
    out.extend(["-lws2_32", "-lcrypt32", "-lgdi32", "-ladvapi32", "-lbcrypt", "-luser32"])
print(" ".join(out))
PY
}

run_one() {
  local name="$1"
  local program="$2"
  local module_root="$3"
  local shim="$4"

  echo "run-os smoke: $name"
  local pkg_dir
  pkg_dir="$(cd "$(dirname "$module_root")" && pwd)"
  local libs
  libs="$(ffi_lib_args "$pkg_dir/x07-package.json")"
  X07_CC_ARGS="$shim $libs" ./target/debug/x07-os-runner \
    --program "$program" \
    --world run-os \
    --module-root "$test_modules" \
    --module-root "$module_root" \
    | "$python_bin" scripts/ci/assert_run_os_ok.py "$name"
}

run_one_multi() {
  local name="$1"
  local program="$2"
  local package_manifest="$3"
  local shim="$4"
  shift 4

  echo "run-os smoke: $name"
  local libs
  libs="$(ffi_lib_args "$package_manifest")"
  local module_root_args=(--module-root "$test_modules")
  for r in "$@"; do
    module_root_args+=(--module-root "$r")
  done
  X07_CC_ARGS="$shim $libs" ./target/debug/x07-os-runner \
    --program "$program" \
    --world run-os \
    "${module_root_args[@]}" \
    | "$python_bin" scripts/ci/assert_run_os_ok.py "$name"
}

run_one_multi_sandboxed() {
  local name="$1"
  local program="$2"
  local policy="$3"
  local package_manifest="$4"
  local shim="$5"
  shift 5

  echo "run-os-sandboxed smoke: $name"
  local libs
  libs="$(ffi_lib_args "$package_manifest")"
  local module_root_args=(--module-root "$test_modules")
  for r in "$@"; do
    module_root_args+=(--module-root "$r")
  done
  X07_CC_ARGS="$shim $libs" ./target/debug/x07-os-runner \
    --program "$program" \
    --world run-os-sandboxed \
    --policy "$policy" \
    "${module_root_args[@]}" \
    | "$python_bin" scripts/ci/assert_run_os_ok.py "$name"
}

with_tls_echo_server() {
  local host="$1"
  local port="$2"
  shift 2

  local log_path="tmp/tmp/tls_echo_server_${port}.log"
  "$python_bin" "tests/external_os/net_tls/tls_echo_server.py" \
    --host "$host" \
    --port "$port" \
    --timeout-s 20 \
    >"$log_path" 2>&1 &
  local pid="$!"

  cleanup() {
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
  }
  trap cleanup RETURN

  # Avoid flaky sleeps: wait until the server is listening before running the client.
  ready="false"
  for _i in $(seq 1 200); do
    if "$python_bin" - "$host" "$port" <<'PY'
import socket, sys
host = sys.argv[1]
port = int(sys.argv[2])
try:
    with socket.create_connection((host, port), timeout=0.2):
        pass
except OSError:
    raise SystemExit(1)
raise SystemExit(0)
PY
    then
      ready="true"
      break
    fi
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      echo "ERROR: tls_echo_server exited early; log follows:" >&2
      cat "$log_path" >&2 || true
      exit 1
    fi
    sleep 0.05
  done
  if [[ "$ready" != "true" ]]; then
    echo "ERROR: tls_echo_server did not become ready (timeout); log follows:" >&2
    cat "$log_path" >&2 || true
    exit 1
  fi

  "$@"
}

run_one_multi_bg_with_http_client() {
  local name="$1"
  local world="$2"
  local program="$3"
  local policy="$4"
  local package_manifest="$5"
  local shim="$6"
  local host="$7"
  local port="$8"
  shift 8

  echo "$world smoke: $name"
  local libs
  libs="$(ffi_lib_args "$package_manifest")"
  local module_root_args=(--module-root "$test_modules")
  for r in "$@"; do
    module_root_args+=(--module-root "$r")
  done

  local report="tmp/tmp/http_server_${world}_${port}.json"
  local stderr_path="${report}.stderr"
  local client_timeout_s=20
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) client_timeout_s=60 ;;
  esac
  local runner_timeout_s=30
  if (( runner_timeout_s < client_timeout_s + 5 )); then
    runner_timeout_s=$((client_timeout_s + 5))
  fi

  if [[ -n "$policy" ]]; then
    X07_CC_ARGS="$shim $libs" ./target/debug/x07-os-runner \
      --cpu-time-limit-seconds "$runner_timeout_s" \
      --program "$program" \
      --world "$world" \
      --policy "$policy" \
      "${module_root_args[@]}" \
      >"$report" 2>"$stderr_path" &
  else
    X07_CC_ARGS="$shim $libs" ./target/debug/x07-os-runner \
      --cpu-time-limit-seconds "$runner_timeout_s" \
      --program "$program" \
      --world "$world" \
      "${module_root_args[@]}" \
      >"$report" 2>"$stderr_path" &
  fi
  local pid="$!"

  cleanup() {
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
  }
  trap cleanup RETURN

  if ! "$python_bin" "tests/external_os/net_http_server/http_client.py" \
    --host "$host" \
    --port "$port" \
    --timeout-s "$client_timeout_s" \
    >/dev/null; then
    echo "http client failed: ${name}" >&2
    echo "http server stderr: ${stderr_path}" >&2
    cat "$stderr_path" >&2 || true
    if [[ -s "$report" ]]; then
      echo "http server report: ${report}" >&2
      "$python_bin" scripts/ci/assert_run_os_ok.py "$name" --path "$report" || true
    fi
    return 1
  fi

  wait "$pid" || true
  trap - RETURN

  "$python_bin" scripts/ci/assert_run_os_ok.py "$name" --path "$report"
}

run_one "ext-zlib-c" \
  "tests/external_os/zlib/src/main.x07.json" \
  "$(x07_ext_pkg_modules x07-ext-zlib-c)" \
  "$(x07_ext_pkg_ffi x07-ext-zlib-c zlib_shim.c)"

run_one "ext-openssl-c" \
  "tests/external_os/openssl/src/main.x07.json" \
  "$(x07_ext_pkg_modules x07-ext-openssl-c)" \
  "$(x07_ext_pkg_ffi x07-ext-openssl-c openssl_shim.c)"

run_one "ext-curl-c" \
  "tests/external_os/curl/src/main.x07.json" \
  "$(x07_ext_pkg_modules x07-ext-curl-c)" \
  "$(x07_ext_pkg_ffi x07-ext-curl-c curl_shim.c)"

run_one_multi "ext-net" \
  "tests/external_os/net/src/main.x07.json" \
  "$(x07_ext_pkg_manifest x07-ext-curl-c)" \
  "$(x07_ext_pkg_ffi x07-ext-curl-c curl_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-curl-c)"

run_one_multi_sandboxed "ext-net (file:// only)" \
  "tests/external_os/net/src/main.x07.json" \
  "tests/external_os/net/run-os-policy.file-etc-allow-ffi.json" \
  "$(x07_ext_pkg_manifest x07-ext-curl-c)" \
  "$(x07_ext_pkg_ffi x07-ext-curl-c curl_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-curl-c)"

run_one_multi "ext-net sockets" \
  "tests/external_os/net_sockets/src/main.x07.json" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)"

run_one_multi_sandboxed "ext-net sockets (loopback allow)" \
  "tests/external_os/net_sockets/src/main.x07.json" \
  "tests/external_os/net_sockets/run-os-policy.loopback-allow.json" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)"

run_one_multi "ext-net iface streaming" \
  "tests/external_os/net_iface_stream/src/main.x07.json" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)"

run_one_multi_sandboxed "ext-net iface streaming (loopback allow)" \
  "tests/external_os/net_iface_stream/src/main.x07.json" \
  "tests/external_os/net_sockets/run-os-policy.loopback-allow.json" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)"

run_one_multi_bg_with_http_client "ext-net http server" \
  "run-os" \
  "tests/external_os/net_http_server/src/main.x07.json" \
  "" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "127.0.0.1" "30031" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)" \
  "$(x07_ext_pkg_modules x07-ext-url-rs)"

run_one_multi_bg_with_http_client "ext-net http server (loopback allow)" \
  "run-os-sandboxed" \
  "tests/external_os/net_http_server/src/main_30032.x07.json" \
  "tests/external_os/net_sockets/run-os-policy.loopback-allow.json" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "127.0.0.1" "30032" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)" \
  "$(x07_ext_pkg_modules x07-ext-url-rs)"

with_tls_echo_server "127.0.0.1" "30030" \
  run_one_multi "ext-net tls" \
    "tests/external_os/net_tls/src/main.x07.json" \
    "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
    "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
    "$(x07_ext_pkg_modules x07-ext-net)" \
    "$(x07_ext_pkg_modules x07-ext-sockets-c)"

with_tls_echo_server "127.0.0.1" "30030" \
  run_one_multi_sandboxed "ext-net tls (loopback allow)" \
    "tests/external_os/net_tls/src/main.x07.json" \
    "tests/external_os/net_sockets/run-os-policy.loopback-allow.json" \
    "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
    "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
    "$(x07_ext_pkg_modules x07-ext-net)" \
    "$(x07_ext_pkg_modules x07-ext-sockets-c)"
run_one_multi_sandboxed "ext-net sockets (policy denied)" \
  "tests/external_os/net_sockets_policy_denied/src/main.x07.json" \
  "tests/external_os/net/run-os-policy.file-etc-allow-ffi.json" \
  "$(x07_ext_pkg_manifest x07-ext-sockets-c)" \
  "$(x07_ext_pkg_ffi x07-ext-sockets-c sockets_shim.c)" \
  "$(x07_ext_pkg_modules x07-ext-net)" \
  "$(x07_ext_pkg_modules x07-ext-sockets-c)"

echo "ok: external OS-world packages smoke"
