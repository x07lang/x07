#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

export X07_SANDBOX_BACKEND="${X07_SANDBOX_BACKEND:-os}"
export X07_I_ACCEPT_WEAKER_ISOLATION="${X07_I_ACCEPT_WEAKER_ISOLATION:-1}"
export X07_OFFLINE="${X07_OFFLINE:-1}"

./scripts/ci/check_tools.sh >/dev/null
./scripts/ci/ensure_math_backend.sh >/dev/null
./scripts/ci/ensure_stream_xf_backend.sh >/dev/null

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "$x07_bin" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

ext_roots=(
  "packages/ext/x07-ext-archive-c/0.1.5/modules"
  "packages/ext/x07-ext-base64-rs/0.1.4/modules"
  "packages/ext/x07-ext-compress-rs/0.1.5/modules"
  "packages/ext/x07-ext-data-model/0.1.11/modules"
  "packages/ext/x07-ext-json-rs/0.1.7/modules"
  "packages/ext/x07-ext-unicode-rs/0.1.5/modules"
)

ext_root_args=()
for r in "${ext_roots[@]}"; do
  ext_root_args+=(--module-root "$r")
done

"$x07_bin" test \
  --manifest packages/ext/x07-ext-archive-c/0.1.5/tests/tests.json \
  --no-fail-fast \
  --json=false \
  "${ext_root_args[@]}"

"$x07_bin" test \
  --manifest tests/corpora/archive/tests.json \
  --module-root tests/corpora/archive/modules \
  --no-fail-fast \
  --json=false \
  "${ext_root_args[@]}"

echo "ok: archive corpus suite passed"

