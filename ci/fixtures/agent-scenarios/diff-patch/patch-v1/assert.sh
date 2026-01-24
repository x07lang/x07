#!/usr/bin/env bash
set -euo pipefail

scenario_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$scenario_dir/../../_lib.sh"

require_env X07_BIN
require_env X07_PYTHON
require_env X07_REPO_ROOT
require_env X07_AGENT_SCENARIOS_TMP

work="$X07_AGENT_SCENARIOS_TMP/diff-patch/patch-v1"
rm -rf "$work"
mkdir -p "$work"
copy_tree "$scenario_dir/broken" "$work"

(cd "$work" && "$X07_BIN" pkg lock --check --offline --project x07.json >/dev/null)

report="$(run_tests_report "$work")"
normalize_x07test_report_to_golden "$report" "$work/tmp/test.golden.json"
assert_json_golden_eq "diff-patch/patch-v1 (tests)" "$work/tmp/test.golden.json" "$scenario_dir/golden.test.report.json"

wrapped="$(run_wrapped "diff-patch/patch-v1 (run)" "$work" --profile test)"
unwrap_wrapped_report "$wrapped" "$work/tmp/runner.json"

"$X07_PYTHON" - "$work/tmp/runner.json" "$scenario_dir/patch_v1.bin" <<'PY'
import base64
import json
import sys
from pathlib import Path

runner_path = Path(sys.argv[1])
golden_path = Path(sys.argv[2])

doc = json.loads(runner_path.read_text(encoding="utf-8"))
solve = doc.get("solve") or {}
b64s = str(solve.get("solve_output_b64") or "")
got = base64.b64decode(b64s.encode("ascii"), validate=False)
want = golden_path.read_bytes()
if got != want:
    raise SystemExit("patch_v1.bin mismatch vs x07 run output")
PY

rm_ephemeral "$work"
diff_snapshot "$scenario_dir/expected" "$work" >/dev/null

