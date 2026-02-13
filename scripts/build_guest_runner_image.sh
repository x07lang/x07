#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

usage() {
  cat <<'EOF'
usage: scripts/build_guest_runner_image.sh [--image <repo>] [--tag <tag>]

Builds the Linux guest runner OCI image used by sandbox_backend=vm.

Defaults:
  image: ghcr.io/x07lang/x07-guest-runner
  tag:   <workspace x07 crate version>
EOF
}

root="$(repo_root)"
cd "$root"

image="ghcr.io/x07lang/x07-guest-runner"
tag=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --image)
      image="${2:-}"
      shift 2
      ;;
    --tag)
      tag="${2:-}"
      shift 2
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
  tag="$(
    python3 - <<'PY'
import pathlib, re, sys
p = pathlib.Path("crates/x07/Cargo.toml")
txt = p.read_text(encoding="utf-8")
m = re.search(r'(?m)^version\\s*=\\s*\"([^\"]+)\"\\s*$', txt)
if not m:
  print("", end="")
  sys.exit(0)
print(m.group(1), end="")
PY
  )"
fi

if [[ -z "$tag" ]]; then
  echo "ERROR: could not determine x07 version for image tag" >&2
  exit 2
fi

docker build \
  -f ci/x07-guest-runner/Dockerfile \
  -t "${image}:${tag}" \
  .

echo "ok: built ${image}:${tag}"

