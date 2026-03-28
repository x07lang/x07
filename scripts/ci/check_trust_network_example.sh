#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: check_trust_network_example.sh \
  --label <label> \
  --example-dir <relative-path> \
  --template <template-name> \
  --profile <relative-path> \
  --entry <symbol>
EOF
  exit 2
}

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

copy_review_artifacts() {
  local label="$1"
  local variant="$2"
  local cert_dir="$3"
  local review_root="${X07_REVIEW_ARTIFACTS_DIR:-}"
  if [[ -z "$review_root" || ! -d "$cert_dir" ]]; then
    return
  fi
  local dest="$review_root/$label/$variant"
  rm -rf "$dest"
  mkdir -p "$(dirname "$dest")"
  cp -R "$cert_dir" "$dest"
}

resolve_x07_bin() {
  local root="$1"
  local x07_bin="${X07_BIN:-}"
  if [[ -z "$x07_bin" ]]; then
    x07_bin="$("$root/scripts/ci/find_x07.sh")"
  fi
  if [[ "$x07_bin" != /* ]]; then
    x07_bin="$root/$x07_bin"
  fi
  printf '%s\n' "$x07_bin"
}

require_vm_inputs() {
  local vm_backend="${X07_VM_BACKEND:-}"
  if [[ "$vm_backend" == "vz" ]]; then
    if [[ -z "${X07_VM_VZ_GUEST_BUNDLE:-}" ]]; then
      echo "error: X07_VM_VZ_GUEST_BUNDLE is required for X07_VM_BACKEND=vz" >&2
      exit 2
    fi
    return
  fi

  if [[ -z "${X07_VM_GUEST_IMAGE:-}" ]]; then
    echo "error: X07_VM_GUEST_IMAGE is required for VM certification checks" >&2
    exit 2
  fi
}

require_solvers_if_needed() {
  local skip_certify="$1"
  if [[ "$skip_certify" == "1" ]]; then
    return
  fi
  if ! command -v cbmc >/dev/null 2>&1; then
    echo "error: network trust certify requires cbmc on PATH" >&2
    exit 2
  fi
  if ! command -v z3 >/dev/null 2>&1; then
    echo "error: network trust certify requires z3 on PATH" >&2
    exit 2
  fi
}

run_static_checks() {
  local label="$1"
  local work_dir="$2"
  local x07_bin="$3"
  local profile="$4"
  local entry="$5"

  echo "[check] $label: lock sync"
  (
    cd "$work_dir"
    "$x07_bin" pkg lock --project x07.json >/dev/null
  )

  echo "[check] $label: profile check"
  (
    cd "$work_dir"
    "$x07_bin" trust profile check \
      --project x07.json \
      --profile "$profile" \
      --entry "$entry" \
      >/dev/null
  )

  echo "[check] $label: capsule check"
  (
    cd "$work_dir"
    "$x07_bin" trust capsule check \
      --project x07.json \
      --index arch/capsules/index.x07capsule.json \
      >/dev/null
  )

  echo "[check] $label: dependency closure"
  (
    cd "$work_dir"
    "$x07_bin" pkg attest-closure \
      --project x07.json \
      --out target/dep-closure.attest.json \
      >/dev/null
  )
  test -f "$work_dir/target/dep-closure.attest.json"
}

run_runtime_checks() {
  local label="$1"
  local work_dir="$2"
  local x07_bin="$3"
  local profile="$4"
  local entry="$5"
  local cert_out="$6"
  local skip_certify="$7"

  echo "[check] $label: sandboxed tests"
  (
    cd "$work_dir"
    python3 tests/tcp_echo_server.py --host 127.0.0.1 --port 30030 --timeout-s 120 &
    local server_pid="$!"
    trap 'kill "$server_pid" >/dev/null 2>&1 || true' EXIT
    "$x07_bin" test --all --manifest tests/tests.json >/dev/null
    if [[ "$skip_certify" != "1" ]]; then
      "$x07_bin" trust certify \
        --project x07.json \
        --profile "$profile" \
        --entry "$entry" \
        --out-dir "$cert_out" \
        >/dev/null
    fi
  )

  if [[ "$skip_certify" == "1" ]]; then
    echo "[check] $label: certify skipped (X07_SKIP_CERTIFY=1)"
    return
  fi

  test -f "$cert_out/certificate.json"
  python3 "$root/scripts/ci/assert_strict_certificate.py" \
    --cert "$cert_out/certificate.json" \
    --x07-bin "$x07_bin" \
    --cwd "$work_dir" \
    --label "$label" \
    --require-entry-formally-proved
  local artifact_label="$label"
  local artifact_variant="cert"
  if [[ "$label" == *" docs example" ]]; then
    artifact_label="${label% docs example}"
    artifact_variant="docs-example"
  elif [[ "$label" == *" template" ]]; then
    artifact_label="${label% template}"
    artifact_variant="template"
  fi
  copy_review_artifacts "$artifact_label" "$artifact_variant" "$cert_out"
}

label=""
example_dir_rel=""
template=""
profile=""
entry=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --label)
      label="$2"
      shift 2
      ;;
    --example-dir)
      example_dir_rel="$2"
      shift 2
      ;;
    --template)
      template="$2"
      shift 2
      ;;
    --profile)
      profile="$2"
      shift 2
      ;;
    --entry)
      entry="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

if [[ -z "$label" || -z "$example_dir_rel" || -z "$template" || -z "$profile" || -z "$entry" ]]; then
  usage
fi

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

mode="${X07_NETWORK_EXAMPLE_MODE:-vm}"
skip_certify="${X07_SKIP_CERTIFY:-0}"

case "$mode" in
  static)
    cargo build -p x07 >/dev/null
    ;;
  vm)
    export X07_SANDBOX_BACKEND="vm"
    export X07_I_ACCEPT_WEAKER_ISOLATION="${X07_I_ACCEPT_WEAKER_ISOLATION:-0}"
    require_vm_inputs
    require_solvers_if_needed "$skip_certify"
    cargo build -p x07 -p x07c -p x07-host-runner -p x07-os-runner >/dev/null
    ;;
  *)
    echo "error: unsupported X07_NETWORK_EXAMPLE_MODE: $mode" >&2
    exit 2
    ;;
esac

x07_bin="$(resolve_x07_bin "$root")"
bin_dir="$(cd "$(dirname "$x07_bin")" && pwd)"
export PATH="$bin_dir:$PATH"

case "$(uname -s)" in
  Darwin)
    tmp_dir="$(mktemp -d -t x07_trust_network_example)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

example_dir="$root/$example_dir_rel"
test -f "$example_dir/.github/workflows/certify.yml"

run_static_checks "$label docs example" "$example_dir" "$x07_bin" "$profile" "$entry"

if [[ "$mode" == "vm" ]]; then
  run_runtime_checks \
    "$label docs example" \
    "$example_dir" \
    "$x07_bin" \
    "$profile" \
    "$entry" \
    "$tmp_dir/example-cert" \
    "$skip_certify"
fi

scaffold_dir="$tmp_dir/init"
mkdir -p "$scaffold_dir"

echo "[check] $label template: init"
(
  cd "$scaffold_dir"
  "$x07_bin" init --template "$template" >/dev/null
)
test -f "$scaffold_dir/.github/workflows/certify.yml"

run_static_checks "$label template" "$scaffold_dir" "$x07_bin" "$profile" "$entry"

if [[ "$mode" == "vm" ]]; then
  run_runtime_checks \
    "$label template" \
    "$scaffold_dir" \
    "$x07_bin" \
    "$profile" \
    "$entry" \
    "$scaffold_dir/target/cert" \
    "$skip_certify"
fi

printf 'OK %s\n' "$(basename "$0")"
