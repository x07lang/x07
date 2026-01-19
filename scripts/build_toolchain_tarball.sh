#!/usr/bin/env bash
set -euo pipefail

umask 022

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

usage() {
  cat <<'EOF'
Usage:
  scripts/build_toolchain_tarball.sh --tag vX.Y.Z [--platform <macOS|Linux>] [--target-dir <dir>] [--out <path>]

Builds a toolchain tarball containing:
  - bin/{x07,x07c,x07-host-runner,x07-os-runner,x07import-cli}
  - stdlib/os/0.2.0/modules (for x07-os-runner)

Expected inputs:
  - Release binaries already built under <target-dir>/release (default: ./target/release)

Examples:
  cargo build --release -p x07 -p x07c -p x07-host-runner -p x07-os-runner -p x07import-cli
  scripts/build_toolchain_tarball.sh --tag v0.0.5 --platform macOS
EOF
}

root="$(repo_root)"

tag=""
platform=""
target_dir="${CARGO_TARGET_DIR:-$root/target}"
out=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      tag="${2:-}"
      shift 2
      ;;
    --platform)
      platform="${2:-}"
      shift 2
      ;;
    --target-dir)
      target_dir="${2:-}"
      shift 2
      ;;
    --out)
      out="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$tag" ]]; then
  echo "ERROR: --tag is required" >&2
  usage >&2
  exit 2
fi

if [[ -z "$platform" ]]; then
  case "$(uname -s)" in
    Darwin) platform="macOS" ;;
    Linux) platform="Linux" ;;
    *) platform="unknown" ;;
  esac
fi

if [[ "$platform" != "macOS" && "$platform" != "Linux" ]]; then
  echo "ERROR: unsupported platform label: $platform (expected macOS or Linux)" >&2
  exit 2
fi

if [[ -z "$out" ]]; then
  out="$root/dist/x07-${tag}-${platform}.tar.gz"
fi

stage_root="$root/dist/.tmp_toolchain_${tag}_${platform}"
rm -rf "$stage_root"
mkdir -p "$stage_root/bin"
mkdir -p "$stage_root/stdlib/os/0.2.0"

install_bin() {
  local name="$1"
  local src="$target_dir/release/$name"
  if [[ ! -f "$src" ]]; then
    echo "ERROR: missing build output: $src" >&2
    exit 1
  fi

  local dst="$stage_root/bin/$name"
  if command -v install >/dev/null 2>&1; then
    install -m 0755 "$src" "$dst"
  else
    cp -f "$src" "$dst"
    chmod 0755 "$dst"
  fi

  touch -t 200001010000.00 "$dst" 2>/dev/null || true
}

for bin in x07 x07c x07-host-runner x07-os-runner x07import-cli; do
  install_bin "$bin"
done

stdlib_src="$root/stdlib/os/0.2.0/modules"
stdlib_dst="$stage_root/stdlib/os/0.2.0/modules"
if [[ ! -d "$stdlib_src" ]]; then
  echo "ERROR: missing stdlib dir: $stdlib_src" >&2
  exit 1
fi
cp -R "$stdlib_src" "$stdlib_dst"

find "$stage_root" -exec touch -t 200001010000.00 {} + 2>/dev/null || true

mkdir -p "$(dirname "$out")"
tar -czf "$out" -C "$stage_root" .
echo "ok: wrote $out"

