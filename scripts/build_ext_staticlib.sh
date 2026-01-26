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

cargo_cmd="${X07_CARGO:-cargo}"
target="${X07_CARGO_TARGET:-}"

host_manifest="$manifest"
manifest_for_build="$manifest"
if [[ "$host_manifest" != /* ]]; then
  host_manifest="$root/$host_manifest"
fi

if [[ "$cargo_cmd" == "cross" ]]; then
  if [[ "$manifest_for_build" == /* ]]; then
    case "$manifest_for_build" in
      "$root"/*)
        manifest_for_build="${manifest_for_build#"$root"/}"
        ;;
      *)
        echo "ERROR: cross build requires --manifest under repo root (got: $manifest_for_build)" >&2
        exit 2
        ;;
    esac
  fi
else
  manifest_for_build="$host_manifest"
fi

host_header="$header"
if [[ "$host_header" != /* ]]; then
  host_header="$root/$host_header"
fi

if [[ ! -f "$host_manifest" ]]; then
  echo "ERROR: manifest not found: $host_manifest" >&2
  exit 2
fi
if [[ ! -f "$host_header" ]]; then
  echo "ERROR: header not found: $host_header" >&2
  exit 2
fi

user_target_dir="${CARGO_TARGET_DIR:-}"
host_target_dir=""
cargo_target_dir_env=""
if [[ "$cargo_cmd" == "cross" ]]; then
  if [[ -n "$user_target_dir" ]]; then
    if [[ "$user_target_dir" == /* ]]; then
      case "$user_target_dir" in
        "$root"/*)
          cargo_target_dir_env="${user_target_dir#"$root"/}"
          host_target_dir="$user_target_dir"
          ;;
        *)
          echo "ERROR: cross build requires CARGO_TARGET_DIR under repo root (got: $user_target_dir)" >&2
          exit 2
          ;;
      esac
    else
      cargo_target_dir_env="$user_target_dir"
      host_target_dir="$root/$user_target_dir"
    fi
  else
    cargo_target_dir_env="target"
    host_target_dir="$root/target"
  fi
else
  if [[ -n "$user_target_dir" ]]; then
    if [[ "$user_target_dir" == /* ]]; then
      host_target_dir="$user_target_dir"
    else
      host_target_dir="$root/$user_target_dir"
    fi
  else
    host_target_dir="$root/target"
  fi
  cargo_target_dir_env="$host_target_dir"
fi

cargo_args=(build --manifest-path "$manifest_for_build" --release)
if [[ -n "$target" ]]; then
  cargo_args+=(--target "$target")
fi

(
  cd "$root"
  CARGO_TARGET_DIR="$cargo_target_dir_env" "$cargo_cmd" "${cargo_args[@]}"
)

build_out="$host_target_dir/release"
if [[ -n "$target" ]]; then
  build_out="$host_target_dir/$target/release"
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

cp -f "$host_header" "$deps_dir/include/$(basename "$host_header")"

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
