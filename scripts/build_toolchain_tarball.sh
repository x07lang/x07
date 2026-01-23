#!/usr/bin/env bash
set -euo pipefail

umask 022

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

usage() {
  cat <<'EOF'
Usage:
  scripts/build_toolchain_tarball.sh --tag vX.Y.Z [--platform <macOS|Linux>] [--target-dir <dir>] [--out <path>] [--skip-native-backends]

Builds a toolchain tarball containing:
  - bin/{x07,x07c,x07-host-runner,x07-os-runner,x07import-cli}
  - stdlib.lock + stdlib.os.lock (stdlib package lockfiles used by `x07 test`)
  - deps/x07/native_backends.json + native backend archives (for native backends like ext-regex)
  - stdlib/os/0.2.0/modules (for x07-os-runner)
  - docs/ (human docs snapshot; also shipped as x07-docs-*.tar.gz)
  - .codex/skills/ (Codex skills pack; also shipped as x07-skills-*.tar.gz)

Expected inputs:
  - Release binaries already built under <target-dir>/release (default: ./target/release)

Examples:
  cargo build --release -p x07 -p x07c -p x07-host-runner -p x07-os-runner -p x07import-cli
  scripts/build_toolchain_tarball.sh --tag v0.0.5 --platform macOS
EOF
}

root="$(repo_root)"

tag=""
platform=""
target_dir="${CARGO_TARGET_DIR:-$root/target}"
out=""
skip_native_backends="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      tag="${2:-}"
      shift 2
      ;;
    --platform)
      platform="${2:-}"
      shift 2
      ;;
    --target-dir)
      target_dir="${2:-}"
      shift 2
      ;;
    --out)
      out="${2:-}"
      shift 2
      ;;
    --skip-native-backends)
      skip_native_backends="true"
      shift 1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$tag" ]]; then
  echo "ERROR: --tag is required" >&2
  usage >&2
  exit 2
fi

if [[ -z "$platform" ]]; then
  case "$(uname -s)" in
    Darwin) platform="macOS" ;;
    Linux) platform="Linux" ;;
    *) platform="unknown" ;;
  esac
fi

if [[ "$platform" != "macOS" && "$platform" != "Linux" ]]; then
  echo "ERROR: unsupported platform label: $platform (expected macOS or Linux)" >&2
  exit 2
fi

if [[ -z "$out" ]]; then
  out="$root/dist/x07-${tag}-${platform}.tar.gz"
fi

stage_root="$root/dist/.tmp_toolchain_${tag}_${platform}"
rm -rf "$stage_root"
mkdir -p "$stage_root/bin"
mkdir -p "$stage_root/deps/x07"
mkdir -p "$stage_root/stdlib/os/0.2.0"
mkdir -p "$stage_root/docs"
mkdir -p "$stage_root/.codex/skills"

install_bin() {
  local name="$1"
  local src="$target_dir/release/$name"
  if [[ ! -f "$src" ]]; then
    echo "ERROR: missing build output: $src" >&2
    exit 1
  fi

  local dst="$stage_root/bin/$name"
  if command -v install >/dev/null 2>&1; then
    install -m 0755 "$src" "$dst"
  else
    cp -f "$src" "$dst"
    chmod 0755 "$dst"
  fi

  touch -t 200001010000.00 "$dst" 2>/dev/null || true
}

for bin in x07 x07c x07-host-runner x07-os-runner x07import-cli; do
  install_bin "$bin"
done

stdlib_src="$root/stdlib/os/0.2.0/modules"
stdlib_dst="$stage_root/stdlib/os/0.2.0/modules"
if [[ ! -d "$stdlib_src" ]]; then
  echo "ERROR: missing stdlib dir: $stdlib_src" >&2
  exit 1
fi
cp -R "$stdlib_src" "$stdlib_dst"

for lock in stdlib.lock stdlib.os.lock; do
  lock_src="$root/$lock"
  if [[ ! -f "$lock_src" ]]; then
    echo "ERROR: missing stdlib lock file: $lock_src" >&2
    exit 1
  fi
  cp -f "$lock_src" "$stage_root/$lock"
done

docs_src="$root/docs"
if [[ ! -d "$docs_src" ]]; then
  echo "ERROR: missing docs dir: $docs_src" >&2
  exit 1
fi
cp -R "$docs_src/." "$stage_root/docs"

skills_src="$root/skills/pack/.codex/skills"
if [[ ! -d "$skills_src" ]]; then
  echo "ERROR: missing skills pack dir: $skills_src" >&2
  exit 1
fi
cp -R "$skills_src/." "$stage_root/.codex/skills"

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  else
    python_bin="python"
  fi
fi

native_backends_src="$root/deps/x07/native_backends.json"
if [[ ! -f "$native_backends_src" ]]; then
  echo "ERROR: missing native backends manifest: $native_backends_src" >&2
  exit 1
fi
cp -f "$native_backends_src" "$stage_root/deps/x07/native_backends.json"

if [[ "$skip_native_backends" == "true" ]]; then
  echo "warn: skipping native backend archives (--skip-native-backends)"
  native_backend_files=""
else
platform_key=""
case "$platform" in
  macOS) platform_key="macos" ;;
  Linux) platform_key="linux" ;;
  *) echo "ERROR: unsupported platform: $platform" >&2; exit 2 ;;
esac

native_backend_files="$("$python_bin" - "$native_backends_src" "$platform_key" <<'PY'
import json
import sys

path = sys.argv[1]
platform_key = sys.argv[2]
doc = json.load(open(path, "r", encoding="utf-8"))
files = []
for backend in doc.get("backends") or []:
    link = backend.get("link") or {}
    spec = link.get(platform_key) or {}
    files.extend(spec.get("files") or [])
for rel in sorted(set(files)):
    print(rel)
PY
)"
fi

while IFS= read -r rel; do
  [[ -z "$rel" ]] && continue
  if [[ "$rel" = /* || "$rel" == *\\* || "$rel" == *..* ]]; then
    echo "ERROR: invalid native backend relpath in deps/x07/native_backends.json: $rel" >&2
    exit 2
  fi
  src="$root/$rel"
  if [[ ! -f "$src" ]]; then
    echo "ERROR: missing native backend file: $src" >&2
    echo "hint: build and stage native backends (for example: ./scripts/build_ext_regex.sh)" >&2
    exit 1
  fi
  dst="$stage_root/$rel"
  mkdir -p "$(dirname "$dst")"
  cp -f "$src" "$dst"
done <<<"$native_backend_files"

find "$stage_root" -exec touch -t 200001010000.00 {} + 2>/dev/null || true

mkdir -p "$(dirname "$out")"
tar -czf "$out" -C "$stage_root" .
echo "ok: wrote $out"
