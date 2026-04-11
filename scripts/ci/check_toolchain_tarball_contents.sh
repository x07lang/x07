#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

step() {
  echo
  echo "==> $*"
}

pick_python() {
  if [[ -n "${X07_PYTHON:-}" ]]; then
    echo "$X07_PYTHON"
    return
  fi
  if [[ -x ".venv/bin/python" ]]; then
    echo ".venv/bin/python"
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    echo "python3"
    return
  fi
  echo "python"
}

detect_platform_label() {
  case "$(uname -s)" in
    Darwin) echo "macOS" ;;
    Linux) echo "Linux" ;;
    MINGW*|MSYS*|CYGWIN*) echo "Windows" ;;
    *) echo "unknown" ;;
  esac
}

python_bin="$(pick_python)"
platform="$(detect_platform_label)"
if [[ "$platform" == "unknown" ]]; then
  echo "ERROR: unsupported platform for toolchain tarball check: $(uname -s)" >&2
  exit 2
fi

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

step "build release binaries (toolchain tarball prerequisites)"
cargo build --release \
  -p x07 \
  -p x07c \
  -p x07-host-runner \
  -p x07-os-runner \
  -p x07-vm-launcher \
  -p x07-vm-reaper \
  -p x07import-cli

step "build toolchain archive"
tag="v0.0.0-ci"
archive="$tmp/x07-${tag}-${platform}"
case "$platform" in
  Windows) archive="${archive}.zip" ;;
  *) archive="${archive}.tar.gz" ;;
esac

./scripts/build_toolchain_tarball.sh \
  --tag "$tag" \
  --platform "$platform" \
  --out "$archive" \
  --skip-native-backends

step "assert required archive contents"
"$python_bin" - "$archive" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
required = [
    "catalog/diagnostics.json",
    "stdlib.std-core.lock",
]

names = set()
if path.suffix.lower() == ".zip":
    import zipfile
    with zipfile.ZipFile(path) as zf:
        for name in zf.namelist():
            if name.startswith("./"):
                name = name[2:]
            names.add(name.rstrip("/"))
else:
    import tarfile
    with tarfile.open(path, "r:*") as tf:
        for member in tf.getmembers():
            name = member.name
            if name.startswith("./"):
                name = name[2:]
            names.add(name.rstrip("/"))

missing = [p for p in required if p not in names]
if missing:
    raise SystemExit(f"ERROR: toolchain archive missing required paths: {', '.join(missing)}")

print("ok: toolchain archive contains required paths")
PY

