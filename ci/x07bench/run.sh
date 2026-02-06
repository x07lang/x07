#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

image="${X07BENCH_DOCKER_IMAGE:-x07bench-local}"

if [[ $# -eq 0 ]]; then
  echo "usage: ci/x07bench/run.sh bench <subcommand> [args...]" >&2
  echo "example: ci/x07bench/run.sh bench eval --suite labs/x07bench/suites/core_v0/suite.json --oracle" >&2
  exit 2
fi

docker build -f ci/x07bench/Dockerfile -t "$image" .

docker run --rm \
  -e X07BENCH_IN_DOCKER=1 \
  -v "$root:/work" \
  -w /work \
  "$image" \
  "$@"
