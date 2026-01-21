#!/usr/bin/env bash
set -euo pipefail

x07_ext_pkg_latest_version() {
  local pkg="$1"
  local dir="packages/ext/${pkg}"
  if [[ ! -d "$dir" ]]; then
    echo "ERROR: missing package dir: ${dir}" >&2
    exit 1
  fi

  local latest
  latest="$(
    ls -1 "$dir" \
      | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' \
      | sort -V \
      | tail -n 1
  )"
  if [[ -z "${latest}" ]]; then
    echo "ERROR: no version dirs found under: ${dir}" >&2
    exit 1
  fi
  echo "$latest"
}

x07_ext_pkg_dir() {
  local pkg="$1"
  echo "packages/ext/${pkg}/$(x07_ext_pkg_latest_version "$pkg")"
}

x07_ext_pkg_modules() {
  local pkg="$1"
  echo "$(x07_ext_pkg_dir "$pkg")/modules"
}

x07_ext_pkg_manifest() {
  local pkg="$1"
  echo "$(x07_ext_pkg_dir "$pkg")/x07-package.json"
}

x07_ext_pkg_ffi() {
  local pkg="$1"
  local rel="$2"
  echo "$(x07_ext_pkg_dir "$pkg")/ffi/${rel}"
}
