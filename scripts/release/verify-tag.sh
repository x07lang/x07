#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: verify-tag.sh --repo-kind <x07|x07-wasm-backend|x07-web-ui|x07-device-host> --tag <vX.Y.Z>
EOF
  exit 2
}

ROOT_DIR="$(pwd)"

repo_kind=""
tag=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo-kind)
      repo_kind="${2:-}"
      shift 2
      ;;
    --tag)
      tag="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      if [[ -z "$tag" && -z "$repo_kind" && "$1" == v* ]]; then
        tag="$1"
        repo_kind="$(basename "$(pwd)")"
        shift
      else
        echo "unknown argument: $1" >&2
        usage
      fi
      ;;
  esac
done

[[ -n "$repo_kind" && -n "$tag" ]] || usage
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]] || {
  echo "invalid --tag: $tag" >&2
  exit 2
}

version="${tag#v}"

read_cargo_version() {
  local cargo_toml="$1"
  python3 - "$cargo_toml" <<'PY'
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
in_package = False
for line in path.read_text(encoding="utf-8").splitlines():
    stripped = line.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        in_package = stripped == "[package]"
        continue
    if not in_package:
        continue
    m = re.match(r'^version\s*=\s*"([^"]+)"\s*$', stripped)
    if m:
        print(m.group(1))
        raise SystemExit(0)
raise SystemExit(f"missing [package].version in {path}")
PY
}

check_equal() {
  local label="$1"
  local got="$2"
  if [[ "$got" != "$version" ]]; then
    echo "${label} version mismatch: tag=${version} file=${got}" >&2
    exit 1
  fi
}

require_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "missing required release file: $path" >&2
    exit 1
  fi
}

case "$repo_kind" in
  x07)
    check_equal "x07" "$(read_cargo_version "$ROOT_DIR/crates/x07/Cargo.toml")"
    require_file "$ROOT_DIR/releases/compat/${version}.json"
    require_file "$ROOT_DIR/releases/bundles/${version}.input.json"
    ;;
  x07-wasm-backend)
    check_equal "x07-wasm" "$(read_cargo_version "$ROOT_DIR/crates/x07-wasm/Cargo.toml")"
    ;;
  x07-web-ui)
    [[ -f "$ROOT_DIR/VERSION" ]] || { echo "missing VERSION" >&2; exit 1; }
    check_equal "x07-web-ui" "$(tr -d '\r\n' < "$ROOT_DIR/VERSION")"
    ;;
  x07-device-host)
    check_equal "x07-device-host-assets" "$(read_cargo_version "$ROOT_DIR/crates/x07-device-host-assets/Cargo.toml")"
    check_equal "x07-device-host-abi" "$(read_cargo_version "$ROOT_DIR/crates/x07-device-host-abi/Cargo.toml")"
    check_equal "x07-device-host-desktop" "$(read_cargo_version "$ROOT_DIR/crates/x07-device-host-desktop/Cargo.toml")"
    ;;
  *)
    echo "unsupported --repo-kind: $repo_kind" >&2
    exit 2
    ;;
esac

printf '%s\n' "$tag"
