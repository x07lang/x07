#!/usr/bin/env bash
set -euo pipefail

scenario_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$scenario_dir/../../_lib.sh"

require_env X07_BIN
require_env X07_PYTHON
require_env X07_REPO_ROOT
require_env X07_AGENT_SCENARIOS_TMP

work="$X07_AGENT_SCENARIOS_TMP/cli-app/repair"
rm -rf "$work"
mkdir -p "$work"
copy_tree "$scenario_dir/broken" "$work"

# Broken state must fail lint in solve-pure (OS import is forbidden).
set +e
lint_one "$work" "solve-pure" "src/app.x07.json" >/dev/null
code="$?"
set -e
if [[ "$code" -eq 0 ]]; then
  die "expected lint failure for broken src/app.x07.json"
fi

fix_one "$work" "solve-pure" "src/app.x07.json"
fmt_write_all "$work"
fmt_check_all "$work"
lint_one "$work" "solve-pure" "src/app.x07.json" >/dev/null

mkdir -p "$work/tmp"
printf "%s\n%s\n%s\n" "https://example.invalid/" "2" "out/results.txt" >"$work/tmp/input.txt"

wrapped="$(run_wrapped "cli-app" "$work" --profile test --input tmp/input.txt)"
unwrap_wrapped_report "$wrapped" "$work/tmp/runner.json"

expected=$'url=https://example.invalid/\ndepth=2\nout=out/results.txt\n'
assert_solve_output "cli-app" "$work/tmp/runner.json" "$expected"

run_tests "$work"

rm_ephemeral "$work"
diff_snapshot "$scenario_dir/expected" "$work" >/dev/null
