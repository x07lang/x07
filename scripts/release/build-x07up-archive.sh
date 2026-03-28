#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: build-x07up-archive.sh --version <X.Y.Z> --target <TARGET> --out-dir <DIR>
EOF
  exit 2
}

version=""
target=""
out_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
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

[[ -n "$version" && -n "$target" && -n "$out_dir" ]] || usage

case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin)
    archive_ext="tar.gz"
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

bin_path="target/${target}/release/x07up${exe_suffix}"
if [[ ! -f "$bin_path" ]]; then
  bin_path="target/release/x07up${exe_suffix}"
fi
[[ -f "$bin_path" ]] || { echo "missing built binary: $bin_path" >&2; exit 1; }

if [[ "$target" == *-apple-darwin ]]; then
  while IFS= read -r lib; do
    case "$lib" in
      /usr/lib/*|/System/Library/*|/Library/Apple/System/Library/*|@rpath/*|@loader_path/*|@executable_path/*) ;;
      *)
        echo "x07up must not depend on non-system macOS libraries: $lib" >&2
        exit 1
        ;;
    esac
  done < <(otool -L "$bin_path" | tail -n +2 | awk '{print $1}')
fi

mkdir -p "$out_dir"
archive_path="${out_dir}/x07up-v${version}-${target}.${archive_ext}"
stage_dir="${out_dir}/.stage/x07up-v${version}-${target}"

rm -rf "$stage_dir" "$archive_path"
mkdir -p "$stage_dir"
cp -f "$bin_path" "${stage_dir}/x07up${exe_suffix}"

if [[ "$archive_ext" == "tar.gz" ]]; then
  tar -czf "$archive_path" -C "$stage_dir" .
else
  python3 - "$stage_dir" "$archive_path" <<'PY'
import pathlib
import sys
import zipfile

stage_dir = pathlib.Path(sys.argv[1]).resolve()
archive_path = pathlib.Path(sys.argv[2]).resolve()

with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
    for path in sorted(stage_dir.rglob("*")):
        if path.is_file():
            zf.write(path, arcname=path.relative_to(stage_dir))
PY
fi

printf '%s\n' "$archive_path"
