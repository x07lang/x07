#!/usr/bin/env bash
set -euo pipefail

umask 022

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

root="$(repo_root)"
cd "$root"

deps_dir="$root/deps/x07"
target_dir="$root/target/os-helpers"

mkdir -p "$deps_dir"

build_one() {
  local pkg="$1"
  echo "[os-helpers] build: $pkg"
  CARGO_TARGET_DIR="$target_dir" cargo build -p "$pkg" --release >/dev/null
}

install_one() {
  local name="$1"

  local src="$target_dir/release/$name"
  local src_exe="$target_dir/release/$name.exe"
  if [[ -f "$src_exe" ]]; then
    src="$src_exe"
  fi

  if [[ ! -f "$src" ]]; then
    echo "[os-helpers] ERROR: build output not found: $src" >&2
    exit 1
  fi

  local dst="$deps_dir/$name"

  if [[ -f "$dst" ]] && cmp -s "$src" "$dst"; then
    : # already up to date
  else
    if [[ -L "$dst" ]]; then
      rm -f "$dst"
    fi
    if command -v install >/dev/null 2>&1; then
      install -m 0755 "$src" "$dst"
    else
      cp -f "$src" "$dst"
      chmod 0755 "$dst"
    fi
  fi

  if [[ "$src" == *.exe ]]; then
    local dst_exe="$deps_dir/$name.exe"
    if command -v install >/dev/null 2>&1; then
      install -m 0755 "$src" "$dst_exe"
    else
      cp -f "$src" "$dst_exe"
      chmod 0755 "$dst_exe"
    fi
    touch -t 200001010000.00 "$dst_exe" 2>/dev/null || true
  fi

  touch -t 200001010000.00 "$dst" 2>/dev/null || true
}

build_one "x07-proc-echo"
build_one "x07-proc-worker-frame-echo"

install_one "x07-proc-echo"
install_one "x07-proc-worker-frame-echo"

echo "[os-helpers] ok"
