#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

export X07_SANDBOX_BACKEND="vm"
export X07_I_ACCEPT_WEAKER_ISOLATION="${X07_I_ACCEPT_WEAKER_ISOLATION:-0}"

if ! command -v cbmc >/dev/null 2>&1; then
  echo "error: trusted-sandbox-program certify requires cbmc on PATH" >&2
  exit 2
fi
if ! command -v z3 >/dev/null 2>&1; then
  echo "error: trusted-sandbox-program certify requires z3 on PATH" >&2
  exit 2
fi

vm_backend="${X07_VM_BACKEND:-}"
if [[ "$vm_backend" == "vz" ]]; then
  if [[ -z "${X07_VM_VZ_GUEST_BUNDLE:-}" ]]; then
    echo "error: X07_VM_VZ_GUEST_BUNDLE is required for X07_VM_BACKEND=vz" >&2
    exit 2
  fi
else
  if [[ -z "${X07_VM_GUEST_IMAGE:-}" ]]; then
    echo "error: X07_VM_GUEST_IMAGE is required for VM certification checks" >&2
    exit 2
  fi
fi

cargo build -p x07 -p x07c -p x07-host-runner -p x07-os-runner >/dev/null

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "${x07_bin}" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

bin_dir="$(cd "$(dirname "$x07_bin")" && pwd)"
export PATH="$bin_dir:$PATH"

case "$(uname -s)" in
  Darwin)
    tmp_dir="$(mktemp -d -t x07_trusted_sandbox_program)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

example_dir="$root/docs/examples/trusted_sandbox_program_v1"
test -f "$example_dir/.github/workflows/certify.yml"

echo "[check] trusted_sandbox_program_v1 docs example: profile check"
(
  cd "$example_dir"
  "$x07_bin" trust profile check \
    --project x07.json \
    --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json \
    --entry example.main \
    >/dev/null
)

echo "[check] trusted_sandbox_program_v1 docs example: capsule check"
(
  cd "$example_dir"
  "$x07_bin" trust capsule check \
    --project x07.json \
    --index arch/capsules/index.x07capsule.json \
    >/dev/null
)

echo "[check] trusted_sandbox_program_v1 docs example: tests"
(
  cd "$example_dir"
  "$x07_bin" test --all --manifest tests/tests.json >/dev/null
)

echo "[check] trusted_sandbox_program_v1 docs example: certify"
(
  cd "$example_dir"
  "$x07_bin" trust certify \
    --project x07.json \
    --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json \
    --entry example.main \
    --out-dir "$tmp_dir/example-cert" \
    >/dev/null
)
cert_path="$tmp_dir/example-cert/certificate.json"
test -f "$cert_path"
python3 ./scripts/ci/assert_strict_certificate.py \
  --cert "$cert_path" \
  --x07-bin "$x07_bin" \
  --cwd "$example_dir" \
  --label trusted_sandbox_program_v1 \
  --require-entry-formally-proved

scaffold_dir="$tmp_dir/init"
mkdir -p "$scaffold_dir"

echo "[check] trusted_sandbox_program_v1 template: init"
(
  cd "$scaffold_dir"
  "$x07_bin" init --template trusted-sandbox-program >/dev/null
)
test -f "$scaffold_dir/.github/workflows/certify.yml"

echo "[check] trusted_sandbox_program_v1 template: profile check"
(
  cd "$scaffold_dir"
  "$x07_bin" trust profile check \
    --project x07.json \
    --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json \
    --entry example.main \
    >/dev/null
)

echo "[check] trusted_sandbox_program_v1 template: capsule check"
(
  cd "$scaffold_dir"
  "$x07_bin" trust capsule check \
    --project x07.json \
    --index arch/capsules/index.x07capsule.json \
    >/dev/null
)

echo "[check] trusted_sandbox_program_v1 template: tests"
(
  cd "$scaffold_dir"
  "$x07_bin" test --all --manifest tests/tests.json >/dev/null
)

echo "[check] trusted_sandbox_program_v1 template: certify"
(
  cd "$scaffold_dir"
  "$x07_bin" trust certify \
    --project x07.json \
    --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json \
    --entry example.main \
    --out-dir target/cert \
    >/dev/null
)
cert_path="$scaffold_dir/target/cert/certificate.json"
test -f "$cert_path"
python3 ./scripts/ci/assert_strict_certificate.py \
  --cert "$cert_path" \
  --x07-bin "$x07_bin" \
  --cwd "$scaffold_dir" \
  --label trusted_sandbox_program_v1_template \
  --require-entry-formally-proved

printf 'OK %s\n' "$(basename "$0")"
