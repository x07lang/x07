# `x07-messaging-rr-lab`

Reference implementation for **Prompt 9** in [`docs/agent/readiness-checks.md`](../../../agent/readiness-checks.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Hydrate deps: `x07 pkg lock --project x07.json`
- Run (record): `x07 run --profile os_record --allow-host 127.0.0.1:9092 -- <args...>` (Kafka) or `--allow-host 127.0.0.1:5672` (AMQP)
- Run (replay): `x07 run --profile rr_replay --fixture-rr-dir .x07_rr -- <args...>`
- Contracts check: `x07 arch check --write-lock`
- Tests: `x07 test --manifest tests/tests.json` (replay only)
