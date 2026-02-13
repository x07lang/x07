#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

root="$(repo_root)"

out="${1:-$root/target/release/x07-vz-helper}"
src="$root/tools/x07-vz-helper/main.swift"
entitlements="$root/tools/x07-vz-helper/entitlements.plist"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: x07-vz-helper can only be built on macOS" >&2
  exit 2
fi

if ! command -v swiftc >/dev/null 2>&1; then
  echo "ERROR: missing swiftc (install Xcode Command Line Tools)" >&2
  exit 2
fi

mkdir -p "$(dirname "$out")"

swiftc -O -o "$out" "$src" -framework Virtualization

if ! command -v codesign >/dev/null 2>&1; then
  echo "ERROR: missing codesign" >&2
  exit 2
fi

codesign --force --sign - --entitlements "$entitlements" "$out"
echo "ok: built $out"

