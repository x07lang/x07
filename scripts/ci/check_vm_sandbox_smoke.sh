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

cargo build -q -p x07 -p x07-os-runner -p x07-vm-launcher -p x07-vm-reaper >/dev/null

x07_bin="$root/target/debug/x07"
if [[ ! -x "$x07_bin" ]]; then
  echo "ERROR: missing x07 binary at $x07_bin" >&2
  exit 2
fi

vm_backend="${X07_VM_BACKEND:-}"
if [[ "$vm_backend" == "vz" ]]; then
  if [[ -z "${X07_VM_VZ_GUEST_BUNDLE:-}" ]]; then
    echo "ERROR: X07_VM_VZ_GUEST_BUNDLE is required for X07_VM_BACKEND=vz" >&2
    echo "hint: build locally:" >&2
    echo "  ./scripts/build_guest_runner_image.sh --image x07-guest-runner --tag vm-smoke" >&2
    echo "  ./scripts/build_vz_guest_bundle.sh --image x07-guest-runner:vm-smoke --out /tmp/x07-guest.bundle" >&2
    echo "  export X07_VM_VZ_GUEST_BUNDLE=/tmp/x07-guest.bundle" >&2
    echo "hint: build the helper binary:" >&2
    echo "  ./scripts/build_vz_helper.sh ./target/debug/x07-vz-helper" >&2
    exit 2
  fi
else
  if [[ -z "${X07_VM_GUEST_IMAGE:-}" ]]; then
    echo "ERROR: X07_VM_GUEST_IMAGE is required for VM smoke (for example: ghcr.io/x07lang/x07-guest-runner:<version>)" >&2
    echo "hint: build locally: ./scripts/build_guest_runner_image.sh --image x07-guest-runner --tag vm-smoke && export X07_VM_GUEST_IMAGE=x07-guest-runner:vm-smoke" >&2
    exit 2
  fi
fi

tmp_dir="$(mktemp -t x07_vm_smoke_XXXXXX -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

fixture_src="$root/ci/fixtures/bundle/async-only"
fixture="$tmp_dir/async-only"
cp -R "$fixture_src" "$fixture"
cd "$fixture"

echo "==> policy init (sandbox base policy)"
"$x07_bin" policy init --template worker --project x07.json >/dev/null

echo "==> run (run-os-sandboxed; sandbox_backend=vm)"
"$x07_bin" run --project x07.json --profile sandbox --input /dev/null >/dev/null

echo "==> security smoke (fs policy deny)"
os_runner_bin="$root/target/debug/x07-os-runner"
if [[ ! -x "$os_runner_bin" ]]; then
  echo "ERROR: missing x07-os-runner binary at $os_runner_bin" >&2
  exit 2
fi

"$os_runner_bin" \
  --program "$root/tests/external_os/fs_policy_deny_smoke/src/main.x07.json" \
  --world run-os-sandboxed \
  --policy "$root/tests/external_os/fs_policy_deny_smoke/run-os-policy.fs_policy_deny_smoke.json" \
  --module-root "$root/tests/external_os/modules" \
  | python3 "$root/scripts/ci/assert_run_os_ok.py" "vm.fs_policy_deny_smoke" --expect "OK" >/dev/null

echo "==> bundle (run-os-sandboxed; sandbox_backend=vm)"
outdir="$tmp_dir/out"
mkdir -p "$outdir"
out_bin="$outdir/async-only"
emit_dir="$outdir/emit"
mkdir -p "$emit_dir"

"$x07_bin" bundle --project x07.json --profile sandbox --out "$out_bin" --emit-dir "$emit_dir" >/dev/null

if [[ ! -x "$out_bin" ]]; then
  echo "ERROR: missing VM bundle binary: $out_bin" >&2
  exit 2
fi
chmod 0755 "$out_bin" >/dev/null 2>&1 || true

echo "==> run bundled binary"
stdout="$outdir/stdout.bin"
stderr="$outdir/stderr.txt"
rm -f "$stdout" "$stderr" || true

if ! "$out_bin" >"$stdout" 2>"$stderr"; then
  echo "ERROR: VM bundle run failed" >&2
  if [[ -f "$stderr" ]]; then
    echo "stderr:" >&2
    cat "$stderr" >&2 || true
  fi
  exit 1
fi

if [[ "$(cat "$stdout" 2>/dev/null || true)" != "ok" ]]; then
  echo "ERROR: unexpected VM bundle stdout (expected ok)" >&2
  echo "stdout:" >&2
  cat "$stdout" >&2 || true
  exit 1
fi

echo "ok: VM sandbox smoke passed"
