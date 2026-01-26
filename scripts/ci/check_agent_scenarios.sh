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

./scripts/ci/ensure_runners.sh

# OS-world agent scenarios rely on the ext-fs native backend.
./scripts/ci/ensure_ext_fs_backend.sh >/dev/null

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
for group in "$scenarios_dir"/*; do
  [[ -d "$group" ]] || continue
  group_name="$(basename "$group")"
  [[ "$group_name" == _* ]] && continue

  for scenario in "$group"/*; do
    [[ -d "$scenario" ]] || continue
    scenario_name="$(basename "$scenario")"
    [[ "$scenario_name" == _* ]] && continue

    id="$group_name/$scenario_name"
    echo "==> agent scenario: $id"

    [[ -f "$scenario/prompt.md" ]] || { echo "ERROR: $id: missing prompt.md" >&2; fail=1; continue; }
    [[ -d "$scenario/broken" ]] || { echo "ERROR: $id: missing broken/" >&2; fail=1; continue; }
    [[ -d "$scenario/expected" ]] || { echo "ERROR: $id: missing expected/" >&2; fail=1; continue; }
    [[ -f "$scenario/assert.sh" ]] || { echo "ERROR: $id: missing assert.sh" >&2; fail=1; continue; }

    set +e
    bash "$scenario/assert.sh"
    code="$?"
    set -e
    if [[ "$code" -ne 0 ]]; then
      echo "ERROR: agent scenario failed: $id (exit $code)" >&2
      fail=1
      continue
    fi
    echo "ok: agent scenario $id"
  done
done

if [[ "$fail" -ne 0 ]]; then
  exit 1
fi

printf 'OK %s\n' "$(basename "$0")"
