# `x07-rr-pipes-smoke`

Reference implementation for **Prompt 7** in [`docs/agent/readiness-checks.md`](../../../agent/readiness-checks.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Hydrate deps: `x07 pkg lock --project x07.json`
- Run (record): `x07 run --profile os_record --allow-host 127.0.0.1:18080 -- <args...>`
- Run (replay): `x07 run --profile rr_replay --fixture-rr-dir .x07_rr -- <args...>`
- Bundle (replay): `x07 bundle --profile rr_replay --out dist/rr-replay`
- Tests: `x07 test --manifest tests/tests.json`
