# x07bench

`x07bench` is the agent-correctness benchmark harness for X07.

Artifacts:

- `suites/` versioned benchmark suites
- each suite contains `suite.json` + per-instance `instance.json` descriptors

Canonical command surface:

- `x07 bench list`
- `x07 bench validate`
- `x07 bench eval`

Seed suites:

- `suites/core_v0/` (minimal)
- `suites/core_v1/` (expanded; recommended)
