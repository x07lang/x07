#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

archive_first_obj_magic_hex() {
  local lib_path="$1"

  if [[ "$lib_path" != /* ]]; then
    lib_path="$root/$lib_path"
  fi

  if [[ ! -f "$lib_path" ]]; then
    return 1
  fi

  if ! command -v ar >/dev/null 2>&1; then
    return 1
  fi
  if ! command -v dd >/dev/null 2>&1; then
    return 1
  fi
  if ! command -v od >/dev/null 2>&1; then
    return 1
  fi

  local obj_name
  obj_name="$(ar t "$lib_path" | grep -v -e '^__' -e '^/' | head -n 1 || true)"
  if [[ -z "$obj_name" ]]; then
    return 1
  fi

  local tmp_dir
  tmp_dir="$(mktemp -d)"

  (
    cd "$tmp_dir"
    ar x "$lib_path" "$obj_name"
  )

  local magic
  magic="$(dd if="$tmp_dir/$obj_name" bs=1 count=4 2>/dev/null | od -An -tx1 | tr -d ' \\n')"

  rm -rf "$tmp_dir"

  if [[ -z "$magic" ]]; then
    return 1
  fi

  echo "$magic"
}

ext_archive_backend_ok_for_host() {
  if [[ -f "deps/x07/x07_ext_archive.lib" ]]; then
    return 0
  fi
  if [[ ! -f "deps/x07/libx07_ext_archive.a" ]]; then
    return 1
  fi

  local magic
  magic="$(archive_first_obj_magic_hex "deps/x07/libx07_ext_archive.a")" || return 1

  case "$(uname -s)" in
    Linux)
      [[ "$magic" == "7f454c46" ]]
      ;;
    Darwin)
      case "$magic" in
        cefaedfe|cffaedfe|feedface|feedfacf|cafebabe|bebafeca) return 0 ;;
        *) return 1 ;;
      esac
      ;;
    *)
      return 1
      ;;
  esac
}

needs_build=0

staged_lib=""
if [[ -f "deps/x07/libx07_ext_archive.a" ]]; then
  staged_lib="deps/x07/libx07_ext_archive.a"
elif [[ -f "deps/x07/x07_ext_archive.lib" ]]; then
  staged_lib="deps/x07/x07_ext_archive.lib"
fi

if [[ ! -f "deps/x07/include/x07_ext_archive_abi_v1.h" ]] || [[ -z "$staged_lib" ]]; then
  needs_build=1
elif ! ext_archive_backend_ok_for_host; then
  needs_build=1
else
  if [[ "crates/x07c/include/x07_ext_archive_abi_v1.h" -nt "deps/x07/include/x07_ext_archive_abi_v1.h" ]]; then
    needs_build=1
  elif [[ "crates/x07-ext-archive-native/Cargo.toml" -nt "$staged_lib" ]] || \
       [[ "crates/x07-ext-archive-native/src/lib.rs" -nt "$staged_lib" ]] || \
       [[ "crates/x07-ext-os-native-core/Cargo.toml" -nt "$staged_lib" ]] || \
       [[ "crates/x07-ext-os-native-core/src/lib.rs" -nt "$staged_lib" ]]; then
    needs_build=1
  fi
fi

if [[ "$needs_build" == "1" ]]; then
  ./scripts/build_ext_staticlib.sh \
    --manifest crates/x07-ext-archive-native/Cargo.toml \
    --lib-name x07_ext_archive \
    --header crates/x07c/include/x07_ext_archive_abi_v1.h >/dev/null
fi
