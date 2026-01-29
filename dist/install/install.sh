#!/bin/sh
set -eu

DEFAULT_ROOT="${HOME:-}/.x07"
DEFAULT_CHANNEL="stable"
DEFAULT_CHANNELS_URL="https://x07lang.org/install/channels.json"
DEFAULT_PROFILE="full"

usage() {
  cat >&2 <<'EOF'
X07 installer (bootstrap).

Downloads x07up, then runs x07up install.

Usage:
  install.sh [--yes] [--root DIR] [--channel NAME | --toolchain TAG] [--channels-url URL]
             [--profile full|minimal] [--no-modify-path] [--quiet] [--json]

Notes:
- In --json mode, stdout is reserved for x07up's JSON report.
- This script is intended for macOS/Linux (Windows is supported via WSL2).
EOF
}

log() {
  if [ "${QUIET}" -eq 0 ] && [ "${JSON}" -eq 0 ]; then
    printf '%s\n' "$*" >&2
  fi
}

warn() { printf '%s\n' "$*" >&2; }
die() { warn "ERROR: $*"; exit 2; }

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    die "missing required command: $1"
  fi
}

pick_downloader() {
  if command -v curl >/dev/null 2>&1; then
    echo curl
    return 0
  fi
  if command -v wget >/dev/null 2>&1; then
    echo wget
    return 0
  fi
  return 1
}

download_to_file() {
  url="$1"
  out="$2"

  if [ "${DOWNLOADER}" = "curl" ]; then
    curl --fail --location --silent --show-error --max-time 600 --output "$out" "$url"
    return 0
  fi
  if [ "${DOWNLOADER}" = "wget" ]; then
    wget -O "$out" "$url"
    return 0
  fi
  die "internal error: unknown downloader: ${DOWNLOADER}"
}

sha256_file() {
  path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | { read -r hash _; printf '%s\n' "$hash"; }
    return 0
  fi
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | { read -r hash _; printf '%s\n' "$hash"; }
    return 0
  fi
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$path" | { read -r _ hash; printf '%s\n' "$hash"; }
    return 0
  fi
  die "missing sha256 tool (need sha256sum, shasum, or openssl)"
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "${os}:${arch}" in
    Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux:aarch64) echo "aarch64-unknown-linux-gnu" ;;
    Linux:arm64) echo "aarch64-unknown-linux-gnu" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    Darwin:arm64) echo "aarch64-apple-darwin" ;;
    *)
      die "unsupported host for installer: os=${os} arch=${arch}"
      ;;
  esac
}

json_get_channel_field() {
  file="$1"
  channel="$2"
  field="$3"

  in_channels=0
  in_entry=0

  while IFS= read -r line; do
    case "$line" in
      '  "channels": {'*)
        in_channels=1
        in_entry=0
        continue
        ;;
    esac

    if [ "$in_channels" -eq 0 ]; then
      continue
    fi

    case "$line" in
      "    \"$channel\": {"*)
        in_entry=1
        continue
        ;;
    esac

    if [ "$in_entry" -eq 0 ]; then
      continue
    fi

    prefix="      \"$field\": \""
    case "$line" in
      "$prefix"*)
        value="${line#"$prefix"}"
        value="${value%%\"*}"
        printf '%s' "$value"
        return 0
        ;;
      '    }'*)
        return 0
        ;;
    esac
  done <"$file"

  return 0
}

json_get_toolchain_min_required_x07up() {
  file="$1"
  toolchain_tag="$2"

  in_toolchains=0
  in_release=0
  in_min=0

  while IFS= read -r line; do
    case "$line" in
      '  "toolchains": {'*)
        in_toolchains=1
        continue
        ;;
    esac

    if [ "$in_toolchains" -eq 0 ]; then
      continue
    fi

    case "$line" in
      "    \"$toolchain_tag\": {"*)
        in_release=1
        continue
        ;;
    esac

    if [ "$in_release" -eq 0 ]; then
      continue
    fi

    case "$line" in
      '      "min_required": {'*)
        in_min=1
        continue
        ;;
    esac

    if [ "$in_min" -eq 0 ]; then
      continue
    fi

    prefix='        "x07up": "'
    case "$line" in
      "$prefix"*)
        value="${line#"$prefix"}"
        value="${value%%\"*}"
        printf '%s' "$value"
        return 0
        ;;
      '      }'*)
        return 0
        ;;
    esac
  done <"$file"

  return 0
}

json_get_x07up_asset_field() {
  file="$1"
  x07up_tag="$2"
  target="$3"
  field="$4"

  in_x07up=0
  in_release=0
  in_assets=0
  in_target=0

  while IFS= read -r line; do
    case "$line" in
      '  "x07up": {'*)
        in_x07up=1
        continue
        ;;
    esac

    if [ "$in_x07up" -eq 0 ]; then
      continue
    fi

    case "$line" in
      "    \"$x07up_tag\": {"*)
        in_release=1
        continue
        ;;
    esac

    if [ "$in_release" -eq 0 ]; then
      continue
    fi

    case "$line" in
      '      "assets": {'*)
        in_assets=1
        continue
        ;;
    esac

    if [ "$in_assets" -eq 0 ]; then
      continue
    fi

    case "$line" in
      "        \"$target\": {"*)
        in_target=1
        continue
        ;;
    esac

    if [ "$in_target" -eq 0 ]; then
      continue
    fi

    prefix="          \"$field\": \""
    case "$line" in
      "$prefix"*)
        value="${line#"$prefix"}"
        value="${value%%\"*}"
        printf '%s' "$value"
        return 0
        ;;
      '        }'*)
        return 0
        ;;
    esac
  done <"$file"

  return 0
}

detect_profile_path() {
  shell="${SHELL:-}"
  shell_name="${shell##*/}"
  case "$shell_name" in
    zsh) echo "${HOME}/.zprofile" ;;
    bash)
      if [ -f "${HOME}/.bash_profile" ]; then
        echo "${HOME}/.bash_profile"
      else
        echo "${HOME}/.profile"
      fi
      ;;
    *) echo "${HOME}/.profile" ;;
  esac
}

maybe_modify_path() {
  if [ "${NO_MODIFY_PATH}" -eq 1 ]; then
    return 0
  fi
  profile_path="$(detect_profile_path)"
  line="export PATH=\"${ROOT}/bin:\$PATH\""
  if [ -f "${profile_path}" ] && grep -F -q "${line}" "${profile_path}"; then
    return 0
  fi
  {
    if [ -f "${profile_path}" ] && [ -s "${profile_path}" ] && [ "$(tail -c 1 "${profile_path}" 2>/dev/null || echo x)" != "" ]; then
      printf '\n'
    fi
    printf '%s\n' "# Added by X07 installer (x07up)"
    printf '%s\n' "${line}"
  } >>"${profile_path}" 2>/dev/null || {
    warn "WARN: failed to modify shell profile (set PATH manually): ${profile_path}"
    return 0
  }
}

ROOT="${DEFAULT_ROOT}"
CHANNEL="${DEFAULT_CHANNEL}"
TOOLCHAIN=""
CHANNELS_URL="${DEFAULT_CHANNELS_URL}"
PROFILE="${DEFAULT_PROFILE}"
YES=0
QUIET=0
JSON=0
NO_MODIFY_PATH=0

while [ $# -gt 0 ]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --yes|-y) YES=1 ;;
    --root)
      shift
      [ $# -gt 0 ] || die "--root requires an argument"
      ROOT="$1"
      ;;
    --channel)
      shift
      [ $# -gt 0 ] || die "--channel requires an argument"
      CHANNEL="$1"
      ;;
    --toolchain)
      shift
      [ $# -gt 0 ] || die "--toolchain requires an argument"
      TOOLCHAIN="$1"
      ;;
    --channels-url)
      shift
      [ $# -gt 0 ] || die "--channels-url requires an argument"
      CHANNELS_URL="$1"
      ;;
    --profile)
      shift
      [ $# -gt 0 ] || die "--profile requires an argument"
      PROFILE="$1"
      ;;
    --no-modify-path) NO_MODIFY_PATH=1 ;;
    --quiet) QUIET=1 ;;
    --json) JSON=1 ;;
    --) shift; break ;;
    *)
      die "unexpected argument: $1 (try --help)"
      ;;
  esac
  shift
done

if [ $# -gt 0 ]; then
  die "unexpected positional arguments (try --help)"
fi

case "${PROFILE}" in
  full|minimal) ;;
  *) die "invalid --profile (expected full|minimal): ${PROFILE}" ;;
esac

need_cmd tar
DOWNLOADER="$(pick_downloader || true)"
if [ -z "${DOWNLOADER}" ]; then
  die "missing downloader (need curl or wget)"
fi

TARGET="$(detect_target)"

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t x07install)"
cleanup() { rm -rf "${TMP_DIR}"; }
trap cleanup EXIT INT TERM

CHANNELS_PATH="${TMP_DIR}/channels.json"
log "fetch channels manifest: ${CHANNELS_URL}"
download_to_file "${CHANNELS_URL}" "${CHANNELS_PATH}"

if [ -n "${TOOLCHAIN}" ]; then
  TOOLCHAIN_TAG="${TOOLCHAIN}"
else
  TOOLCHAIN_TAG="$(json_get_channel_field "${CHANNELS_PATH}" "${CHANNEL}" "toolchain")"
  [ -n "${TOOLCHAIN_TAG}" ] || die "channel not found (or malformed channels.json): ${CHANNEL}"
fi

X07UP_TAG="$(json_get_toolchain_min_required_x07up "${CHANNELS_PATH}" "${TOOLCHAIN_TAG}")"
if [ -z "${X07UP_TAG}" ]; then
  X07UP_TAG="$(json_get_channel_field "${CHANNELS_PATH}" "${CHANNEL}" "x07up")"
fi
[ -n "${X07UP_TAG}" ] || die "could not resolve x07up version from channels.json (channel=${CHANNEL} toolchain=${TOOLCHAIN_TAG})"

X07UP_URL="$(json_get_x07up_asset_field "${CHANNELS_PATH}" "${X07UP_TAG}" "${TARGET}" "url")"
X07UP_SHA="$(json_get_x07up_asset_field "${CHANNELS_PATH}" "${X07UP_TAG}" "${TARGET}" "sha256")"
X07UP_FMT="$(json_get_x07up_asset_field "${CHANNELS_PATH}" "${X07UP_TAG}" "${TARGET}" "format")"

[ -n "${X07UP_URL}" ] || die "x07up asset not found in channels.json (tag=${X07UP_TAG} target=${TARGET})"
[ -n "${X07UP_SHA}" ] || die "x07up sha256 missing in channels.json (tag=${X07UP_TAG} target=${TARGET})"
[ -n "${X07UP_FMT}" ] || die "x07up format missing in channels.json (tag=${X07UP_TAG} target=${TARGET})"

if [ "${X07UP_FMT}" != "tar.gz" ]; then
  die "unsupported x07up archive format: ${X07UP_FMT}"
fi

log "download x07up: ${X07UP_URL}"
X07UP_ARCHIVE="${TMP_DIR}/x07up.tar.gz"
download_to_file "${X07UP_URL}" "${X07UP_ARCHIVE}"

ACTUAL_SHA="$(sha256_file "${X07UP_ARCHIVE}" | tr -d '\r\n' | tr 'A-Z' 'a-z')"
EXPECTED_SHA="$(printf '%s' "${X07UP_SHA}" | tr -d '\r\n' | tr 'A-Z' 'a-z')"
if [ "${ACTUAL_SHA}" != "${EXPECTED_SHA}" ]; then
  die "sha256 mismatch for x07up (expected=${EXPECTED_SHA} got=${ACTUAL_SHA})"
fi

log "extract x07up"
X07UP_EXTRACT_DIR="${TMP_DIR}/x07up"
mkdir -p "${X07UP_EXTRACT_DIR}"
tar -xzf "${X07UP_ARCHIVE}" -C "${X07UP_EXTRACT_DIR}"

X07UP_FOUND=""
if [ -f "${X07UP_EXTRACT_DIR}/x07up" ]; then
  X07UP_FOUND="${X07UP_EXTRACT_DIR}/x07up"
elif [ -f "${X07UP_EXTRACT_DIR}/bin/x07up" ]; then
  X07UP_FOUND="${X07UP_EXTRACT_DIR}/bin/x07up"
elif command -v find >/dev/null 2>&1; then
  X07UP_FOUND="$(find "${X07UP_EXTRACT_DIR}" -type f -name x07up -print | head -n 1 || true)"
fi
[ -n "${X07UP_FOUND}" ] || die "x07up binary not found in archive"

mkdir -p "${ROOT}/bin"
cp "${X07UP_FOUND}" "${ROOT}/bin/x07up"
chmod +x "${ROOT}/bin/x07up"

SPEC="${TOOLCHAIN_TAG}"
if [ -z "${TOOLCHAIN}" ]; then
  SPEC="${CHANNEL}"
fi

X07UP_GLOBAL_ARGS=""
X07UP_GLOBAL_ARGS="${X07UP_GLOBAL_ARGS} --root ${ROOT}"
X07UP_GLOBAL_ARGS="${X07UP_GLOBAL_ARGS} --channels-url ${CHANNELS_URL}"
if [ "${QUIET}" -eq 1 ]; then
  X07UP_GLOBAL_ARGS="${X07UP_GLOBAL_ARGS} --quiet"
fi
if [ "${JSON}" -eq 1 ]; then
  X07UP_GLOBAL_ARGS="${X07UP_GLOBAL_ARGS} --json"
fi

X07UP_INSTALL_ARGS=""
X07UP_INSTALL_ARGS="${X07UP_INSTALL_ARGS} --profile ${PROFILE}"
if [ "${YES}" -eq 1 ]; then
  X07UP_INSTALL_ARGS="${X07UP_INSTALL_ARGS} --yes"
fi

log "install toolchain via x07up: ${SPEC}"
# shellcheck disable=SC2086
"${ROOT}/bin/x07up" ${X07UP_GLOBAL_ARGS} install "${SPEC}" ${X07UP_INSTALL_ARGS}

if [ "${PROFILE}" = "full" ]; then
  # Best-effort: install user skills pack (non-fatal).
  # shellcheck disable=SC2086
  "${ROOT}/bin/x07up" ${X07UP_GLOBAL_ARGS} skills install --user >/dev/null 2>&1 || true
fi

maybe_modify_path

if [ "${JSON}" -eq 0 ]; then
  printf '%s\n' "ok: installed X07 to ${ROOT}" >&2
  printf '%s\n' "next: export PATH=\"${ROOT}/bin:\$PATH\"" >&2
fi
