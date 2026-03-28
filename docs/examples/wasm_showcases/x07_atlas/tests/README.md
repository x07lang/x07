This directory contains the Atlas replay fixtures used by `scripts/ci/check_showcase_fullstack.sh`.

Current contract:
- `tests/traces/happy_path.trace.json` must pass on `atlas_dev`.
- `tests/traces/validation_error.trace.json` must pass on `atlas_dev`.
- `tests/traces/perf_budget.trace.json` must fail on `atlas_budget` with a non-null `incident_dir`.
- `tests/traces/incident_seed.trace.json` must fail on `atlas_dev` with a non-null `incident_dir`.
- `tests/regress/atlas_incident.trace.json` is generated from the intentional mismatch incident and must replay cleanly on `atlas_dev`.

`tests/fixtures/metrics/atlas_canary_ok.json` remains the SLO replay fixture.
