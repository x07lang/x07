#!/usr/bin/env bash
set -euo pipefail

scenario_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$scenario_dir/../../_lib.sh"

require_env X07_BIN
require_env X07_PYTHON
require_env X07_REPO_ROOT
require_env X07_AGENT_SCENARIOS_TMP

work="$X07_AGENT_SCENARIOS_TMP/data-cbor/canonical-encoding"
rm -rf "$work"
mkdir -p "$work"
copy_tree "$scenario_dir/broken" "$work"

(cd "$work" && "$X07_BIN" pkg lock --check --offline --project x07.json >/dev/null)

report="$(run_tests_report "$work")"
normalize_x07test_report_to_golden "$report" "$work/tmp/test.golden.json"
assert_json_golden_eq "data-cbor/canonical-encoding" "$work/tmp/test.golden.json" "$scenario_dir/golden.test.report.json"

rm_ephemeral "$work"
diff_snapshot "$scenario_dir/expected" "$work" >/dev/null

