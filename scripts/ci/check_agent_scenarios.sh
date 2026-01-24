#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi
export X07_PYTHON="$python_bin"

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "$x07_bin" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi
export X07_BIN="$x07_bin"

export X07_REPO_ROOT="$root"

# Ensure runners exist for x07 run.
if [[ ! -x "target/debug/x07-host-runner" && ! -x "target/release/x07-host-runner" ]]; then
  cargo build -p x07-host-runner -p x07-os-runner >/dev/null
fi

scenarios_dir="$root/ci/fixtures/agent-scenarios"
[[ -d "$scenarios_dir" ]] || { echo "ERROR: missing $scenarios_dir" >&2; exit 1; }

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    mkdir -p "$root/tmp"
    tmp_dir="$(mktemp -d -p "$root/tmp" x07_agent_scenarios_XXXXXX)"
    ;;
  *)
    tmp_dir="$(mktemp -t x07_agent_scenarios_XXXXXX -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

export X07_AGENT_SCENARIOS_TMP="$tmp_dir"

fail=0
for s in "$scenarios_dir"/*; do
  [[ -d "$s" ]] || continue
  name="$(basename "$s")"
  [[ "$name" == _* ]] && continue

  echo "==> agent scenario: $name"

  [[ -f "$s/prompt.md" ]] || { echo "ERROR: $name: missing prompt.md" >&2; fail=1; continue; }
  [[ -d "$s/broken" ]] || { echo "ERROR: $name: missing broken/" >&2; fail=1; continue; }
  [[ -d "$s/expected" ]] || { echo "ERROR: $name: missing expected/" >&2; fail=1; continue; }
  [[ -f "$s/assert.sh" ]] || { echo "ERROR: $name: missing assert.sh" >&2; fail=1; continue; }
  [[ -f "$s/golden.report.json" ]] || { echo "ERROR: $name: missing golden.report.json" >&2; fail=1; continue; }

  bash "$s/assert.sh"
  echo "ok: agent scenario $name"
done

if [[ "$fail" -ne 0 ]]; then
  exit 1
fi

printf 'OK %s\n' "$(basename "$0")"

