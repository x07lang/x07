#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: build-core-archive.sh --component x07 --version <X.Y.Z> --target <TARGET> --out-dir <DIR>
EOF
  exit 2
}

component=""
version=""
target=""
out_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --component)
      component="${2:-}"
      shift 2
      ;;
    --version)
      version="${2:-}"
      shift 2
      ;;
    --target)
      target="${2:-}"
      shift 2
      ;;
    --out-dir)
      out_dir="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      ;;
  esac
done

[[ "$component" == "x07" ]] || { echo "--component must be x07" >&2; exit 2; }
[[ -n "$version" && -n "$target" && -n "$out_dir" ]] || usage

tag="v${version}"
case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu)
    platform="Linux"
    target_dir="target/${target}"
    out_name="x07-${version}-${target}.tar.gz"
    ;;
  x86_64-apple-darwin|aarch64-apple-darwin)
    platform="macOS"
    target_dir="target/${target}"
    if [[ ! -d "$target_dir" ]]; then
      target_dir="target"
    fi
    out_name="x07-${version}-${target}.tar.gz"
    ;;
  x86_64-pc-windows-msvc)
    platform="Windows"
    target_dir="target/${target}"
    out_name="x07-${version}-${target}.zip"
    ;;
  *)
    echo "unsupported target for x07 core archive: $target" >&2
    exit 2
    ;;
esac

mkdir -p "$out_dir"
out_path="${out_dir}/${out_name}"

bash scripts/build_toolchain_tarball.sh \
  --tag "$tag" \
  --platform "$platform" \
  --target-dir "$target_dir" \
  --out "$out_path"

printf '%s\n' "$out_path"
