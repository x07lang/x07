# Math package smoke suites (v1)

These JSON files are *shapes* intended for a simple smoke runner:
- compile the program (`program_x07json`)
- run it in the specified world
- compare solver output bytes (`solve_output_b64`) to `expected_b64` for each case
- optionally pass a sandbox policy file (run-os-sandboxed)

Files:
- `ci/suites/smoke/math-f64-bits-smoke.json` (pure, deterministic)
- `ci/suites/smoke/math-f64-api-smoke.json` (pure, broader API coverage)
- `ci/suites/smoke/math-f64-libm-smoke.json` (run-os, requires native backend)

In this repo, `scripts/ci/check_math_smoke.sh` runs these suites:
- `solve-*` via `x07-host-runner`
- `run-os*` via `x07-os-runner`
