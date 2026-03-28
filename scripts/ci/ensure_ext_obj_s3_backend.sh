#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

need_build=0

archive_first_obj_magic_hex() {
  local archive_path="$1"
  if [[ ! -f "$archive_path" ]]; then
    return 1
  fi
  if command -v ar >/dev/null 2>&1; then
    local first
    first="$(ar t "$archive_path" 2>/dev/null | head -n 1 || true)"
    if [[ -z "$first" ]]; then
      return 1
    fi
    ar p "$archive_path" "$first" 2>/dev/null | head -c 4 | xxd -p -c 4
    return 0
  fi
  return 1
}

ext_obj_s3_backend_ok_for_host() {
  if [[ -f "deps/x07/x07_ext_obj_s3.lib" ]]; then
    return 0
  fi
  if [[ ! -f "deps/x07/libx07_ext_obj_s3.a" ]]; then
    return 1
  fi

  local magic
  magic="$(archive_first_obj_magic_hex "deps/x07/libx07_ext_obj_s3.a")" || return 0
  case "$(uname -s)" in
    Darwin)
      [[ "$magic" == "feedfacf" ]] && return 0
      return 1
      ;;
    Linux)
      [[ "$magic" == "7f454c46" ]] && return 0
      return 1
      ;;
    *)
      return 0
      ;;
  esac
}

(
  cd "$ROOT_DIR"

  if [[ ! -f "deps/x07/include/x07_ext_obj_s3_abi_v1.h" ]] || \
     [[ ! -f "deps/x07/libx07_ext_obj_s3.a" && ! -f "deps/x07/x07_ext_obj_s3.lib" ]]; then
    need_build=1
  elif ! ext_obj_s3_backend_ok_for_host; then
    need_build=1
  fi

  if [[ "$need_build" == "1" ]]; then
    ./scripts/build_ext_obj_s3.sh >/dev/null
  fi
)
