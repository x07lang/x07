#!/usr/bin/env bash
set -euo pipefail

scenario_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$scenario_dir/../../_lib.sh"

require_env X07_BIN
require_env X07_PYTHON
require_env X07_REPO_ROOT
require_env X07_AGENT_SCENARIOS_TMP

work="$X07_AGENT_SCENARIOS_TMP/fs-globwalk/missing-dep"
rm -rf "$work"
mkdir -p "$work"
copy_tree "$scenario_dir/broken" "$work"

set +e
wrapped="$(run_wrapped_allow_failure "fs-globwalk/missing-dep (broken)" "$work" --profile os)"
code="$?"
set -e
if [[ "$code" -eq 0 ]]; then
  die "expected broken project to fail x07 run"
fi
assert_wrapped_compile_error_contains "fs-globwalk/missing-dep (broken)" "$wrapped" "x07 pkg add ext-path-glob-rs@0.1.0 --sync"

(cd "$work" && "$X07_BIN" pkg add ext-path-glob-rs@0.1.0 --sync --project x07.json >/dev/null)
(cd "$work" && "$X07_BIN" pkg lock --check --offline --project x07.json >/dev/null)

wrapped_ok="$(run_wrapped "fs-globwalk/missing-dep (fixed)" "$work" --profile os)"
normalize_wrapped_report_to_golden "$wrapped_ok" "$work/tmp/run.golden.json"
assert_json_golden_eq "fs-globwalk/missing-dep (fixed)" "$work/tmp/run.golden.json" "$scenario_dir/golden.run.report.json"

unwrap_wrapped_report "$wrapped_ok" "$work/tmp/runner.json"
assert_solve_output "fs-globwalk/missing-dep (fixed)" "$work/tmp/runner.json" "ok"

rm_ephemeral "$work"
diff_snapshot "$scenario_dir/expected" "$work" >/dev/null

