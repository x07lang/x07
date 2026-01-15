#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

run_one() {
  local name="$1"
  local dockerfile="$2"
  local tag="x07lang-release-${name}"

  echo "=== docker build: ${name} ==="
  docker build --progress=plain -f "${ROOT}/${dockerfile}" -t "${tag}" "${ROOT}"

  echo "=== docker run: ${name} ==="
  docker run --rm "${tag}"
}

run_one "debian" "scripts/examples/docker/Dockerfile.debian"
run_one "ubuntu" "scripts/examples/docker/Dockerfile.ubuntu"
