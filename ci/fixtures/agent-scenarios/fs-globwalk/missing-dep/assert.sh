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

wrapped_ok="$(run_wrapped "fs-globwalk/missing-dep" "$work" --profile os)"
normalize_wrapped_report_to_golden "$wrapped_ok" "$work/tmp/run.golden.json"
assert_json_golden_eq "fs-globwalk/missing-dep" "$work/tmp/run.golden.json" "$scenario_dir/golden.run.report.json"

unwrap_wrapped_report "$wrapped_ok" "$work/tmp/runner.json"
assert_solve_output "fs-globwalk/missing-dep" "$work/tmp/runner.json" "ok"

(cd "$work" && "$X07_BIN" pkg lock --check --offline --project x07.json >/dev/null)

rm_ephemeral "$work"
diff_snapshot "$scenario_dir/expected" "$work" >/dev/null
