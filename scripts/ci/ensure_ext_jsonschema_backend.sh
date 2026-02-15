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

ext_jsonschema_backend_ok_for_host() {
  if [[ -f "deps/x07/x07_ext_jsonschema.lib" ]]; then
    return 0
  fi
  if [[ ! -f "deps/x07/libx07_ext_jsonschema.a" ]]; then
    return 1
  fi

  # Mach-O magic is feedfacf (64-bit) on macOS. On Linux it should be 7f454c46 for ELF in the object.
  # We only do a best-effort check here; if we can't inspect, treat as ok.
  local magic
  magic="$(archive_first_obj_magic_hex "deps/x07/libx07_ext_jsonschema.a")" || return 0
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

  if [[ ! -f "deps/x07/include/x07_ext_jsonschema_abi_v1.h" ]] || \
     [[ ! -f "deps/x07/libx07_ext_jsonschema.a" && ! -f "deps/x07/x07_ext_jsonschema.lib" ]]; then
    need_build=1
  elif ! ext_jsonschema_backend_ok_for_host; then
    need_build=1
  fi

  if [[ "$need_build" == "1" ]]; then
    ./scripts/build_ext_jsonschema.sh >/dev/null
  fi
)
