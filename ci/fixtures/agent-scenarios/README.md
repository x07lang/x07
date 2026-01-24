# Agent scenarios (CI fixtures)

Each scenario is a deterministic “broken -> repair -> expected” project snapshot used by CI to
verify the canonical agent loop:

- `x07 fmt`
- `x07 lint`
- `x07 fix`
- `x07 run`
- `x07 test`

Scenarios are executed by `scripts/ci/check_agent_scenarios.sh`.

