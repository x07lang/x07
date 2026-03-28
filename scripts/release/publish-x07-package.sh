#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: publish-x07-package.sh --package <NAME> --version <X.Y.Z>
                              [--packages-dir <DIR>]
                              [--x07-bin <BIN>]
                              [--index-url <URL>]
                              [--receipt-out <PATH>]
EOF
  exit 2
}

package=""
version=""
packages_dir="packages"
x07_bin="x07"
index_url="sparse+https://registry.x07.io/index/"
receipt_out=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package)
      package="${2:-}"; shift 2 ;;
    --version)
      version="${2:-}"; shift 2 ;;
    --packages-dir)
      packages_dir="${2:-}"; shift 2 ;;
    --x07-bin)
      x07_bin="${2:-}"; shift 2 ;;
    --index-url)
      index_url="${2:-}"; shift 2 ;;
    --receipt-out)
      receipt_out="${2:-}"; shift 2 ;;
    -h|--help)
      usage ;;
    *)
      echo "unknown argument: $1" >&2
      usage ;;
  esac
done

[[ -n "$package" && -n "$version" ]] || usage
pkg_dir="${packages_dir}/${package}/${version}"
[[ -d "$pkg_dir" ]] || { echo "package directory not found: $pkg_dir" >&2; exit 1; }
command -v "$x07_bin" >/dev/null 2>&1 || { echo "x07 binary not found: $x07_bin" >&2; exit 1; }

registry_token="${X07_REGISTRY_TOKEN:-${X07_PKG_TOKEN:-}}"
if [[ -n "$registry_token" ]]; then
  printf '%s' "$registry_token" | "$x07_bin" pkg login --index "$index_url" --token-stdin >/dev/null
fi

tmp_receipt="$(mktemp)"
trap 'rm -f "$tmp_receipt"' EXIT

if ! "$x07_bin" pkg publish --package "$pkg_dir" --index "$index_url" >"$tmp_receipt"; then
  echo "package publish failed: $pkg_dir" >&2
  exit 1
fi

python3 - "$tmp_receipt" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
doc = json.loads(path.read_text(encoding="utf-8"))
if not isinstance(doc, dict):
    raise SystemExit("publish receipt must be a JSON object")
PY

if [[ -n "$receipt_out" ]]; then
  mkdir -p "$(dirname "$receipt_out")"
  mv -f "$tmp_receipt" "$receipt_out"
  trap - EXIT
  printf '%s\n' "$receipt_out"
else
  printf '%s\n' "$pkg_dir"
fi
