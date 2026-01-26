#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

usage() {
  cat <<'EOF' >&2
Usage:
  scripts/build_ext_staticlib.sh --manifest <Cargo.toml> --lib-name <name> --header <header.h>

Builds a Rust `staticlib` crate and stages:
  - deps/x07/include/<header>
  - deps/x07/lib<lib-name>.a   (Unix)
  - deps/x07/<lib-name>.lib    (Windows/MSVC-style)

Environment:
  X07_CARGO: cargo command to use (default: cargo). For cross builds, set X07_CARGO=cross.
  X07_CARGO_TARGET: optional cargo --target triple (for cross builds).
  CARGO_TARGET_DIR: target dir override (default: <repo>/target).
EOF
}

root="$(repo_root)"

manifest=""
lib_name=""
header=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest)
      manifest="${2:-}"
      shift 2
      ;;
    --lib-name)
      lib_name="${2:-}"
      shift 2
      ;;
    --header)
      header="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown arg: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$manifest" || -z "$lib_name" || -z "$header" ]]; then
  echo "ERROR: --manifest, --lib-name, and --header are required" >&2
  usage
  exit 2
fi

if [[ "$manifest" != /* ]]; then
  manifest="$root/$manifest"
fi
if [[ "$header" != /* ]]; then
  header="$root/$header"
fi

if [[ ! -f "$manifest" ]]; then
  echo "ERROR: manifest not found: $manifest" >&2
  exit 2
fi
if [[ ! -f "$header" ]]; then
  echo "ERROR: header not found: $header" >&2
  exit 2
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"

cargo_cmd="${X07_CARGO:-cargo}"
target="${X07_CARGO_TARGET:-}"

cargo_args=(build --manifest-path "$manifest" --release)
if [[ -n "$target" ]]; then
  cargo_args+=(--target "$target")
fi

(
  cd "$root"
  "$cargo_cmd" "${cargo_args[@]}"
)

build_out="$CARGO_TARGET_DIR/release"
if [[ -n "$target" ]]; then
  build_out="$CARGO_TARGET_DIR/$target/release"
fi

lib_candidates=(
  "$build_out/lib${lib_name}.a"
  "$build_out/${lib_name}.lib"
  "$build_out/lib${lib_name}.lib"
)

lib_path=""
for c in "${lib_candidates[@]}"; do
  if [[ -f "$c" ]]; then
    lib_path="$c"
    break
  fi
done

if [[ -z "$lib_path" ]]; then
  echo "ERROR: could not find built static library under: $build_out" >&2
  echo "Tried: ${lib_candidates[*]}" >&2
  exit 2
fi

deps_dir="$root/deps/x07"
mkdir -p "$deps_dir/include"

cp -f "$header" "$deps_dir/include/$(basename "$header")"

staged_lib=""
if [[ "$lib_path" == *.a ]]; then
  staged_lib="$deps_dir/lib${lib_name}.a"
  cp -f "$lib_path" "$staged_lib"
else
  staged_lib="$deps_dir/${lib_name}.lib"
  cp -f "$lib_path" "$staged_lib"
fi

echo "Staged:"
echo "  $deps_dir/include/$(basename "$header")"
echo "  $staged_lib"

