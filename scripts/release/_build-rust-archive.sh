#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: _build-rust-archive.sh \
  --package <cargo-package> \
  --bins <bin1,bin2,...> \
  --asset-prefix <prefix> \
  --version <X.Y.Z> \
  --target <rust-target> \
  --out-dir <DIR>
EOF
  exit 2
}

package=""
bins_csv=""
asset_prefix=""
version=""
target=""
out_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package)
      package="${2:-}"; shift 2 ;;
    --bins)
      bins_csv="${2:-}"; shift 2 ;;
    --asset-prefix)
      asset_prefix="${2:-}"; shift 2 ;;
    --version)
      version="${2:-}"; shift 2 ;;
    --target)
      target="${2:-}"; shift 2 ;;
    --out-dir)
      out_dir="${2:-}"; shift 2 ;;
    -h|--help)
      usage ;;
    *)
      echo "unknown argument: $1" >&2
      usage ;;
  esac
done

[[ -n "$package" && -n "$bins_csv" && -n "$asset_prefix" && -n "$version" && -n "$target" && -n "$out_dir" ]] || usage

case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin)
    archive_ext="tar.xz"
    exe_suffix=""
    ;;
  x86_64-pc-windows-msvc)
    archive_ext="zip"
    exe_suffix=".exe"
    ;;
  *)
    echo "unsupported target: $target" >&2
    exit 2
    ;;
esac

archive_base="${asset_prefix}-${version}-${target}"
archive_path="${out_dir}/${archive_base}.${archive_ext}"
stage_root="${out_dir}/.stage/${archive_base}"

rm -rf "${stage_root}" "${archive_path}"
mkdir -p "${stage_root}/bin"

IFS=',' read -r -a bins <<< "$bins_csv"

cargo_args=(build --locked --release --target "$target" -p "$package")
for bin_name in "${bins[@]}"; do
  cargo_args+=(--bin "$bin_name")
done

echo "building package=${package} target=${target}" >&2
cargo "${cargo_args[@]}"

bin_dir="target/${target}/release"
for bin_name in "${bins[@]}"; do
  src="${bin_dir}/${bin_name}${exe_suffix}"
  [[ -f "$src" ]] || { echo "missing built binary: $src" >&2; exit 1; }
  cp -f "$src" "${stage_root}/bin/"
done

shopt -s nullglob
for lic in LICENSE LICENSE-* LICENSE.* COPYING COPYING.*; do
  if [[ -f "$lic" ]]; then
    cp -f "$lic" "${stage_root}/"
  fi
done
shopt -u nullglob

if [[ -f README.md ]]; then
  cp -f README.md "${stage_root}/"
fi

python3 - "$stage_root" "$archive_path" "$archive_ext" <<'PY'
import pathlib
import sys
import tarfile
import zipfile

stage_root = pathlib.Path(sys.argv[1]).resolve()
archive_path = pathlib.Path(sys.argv[2]).resolve()
archive_ext = sys.argv[3]

archive_path.parent.mkdir(parents=True, exist_ok=True)

if archive_ext == "tar.xz":
    with tarfile.open(archive_path, "w:xz") as tf:
        tf.add(stage_root, arcname=stage_root.name, recursive=True)
elif archive_ext == "zip":
    with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for path in sorted(stage_root.rglob("*")):
            if path.is_file():
                zf.write(path, arcname=str(pathlib.Path(stage_root.name) / path.relative_to(stage_root)))
else:
    raise SystemExit(f"unsupported archive_ext: {archive_ext}")
PY

printf '%s\n' "$archive_path"
