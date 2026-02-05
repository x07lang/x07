# `x07-data-interop-lab`

Reference implementation for **Prompt 3** in [`docs/agent/readiness-checks.md`](../../../agent/readiness-checks.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Run: `x07 run --stdin --profile sandbox`
- Tests: `x07 test --manifest tests/tests.json`
- Stream reader smoke (sandbox): `x07 test --manifest tests/tests.json`

This implementation focuses on deterministic codec interoperability checks in `solve-pure` and
adds one sandboxed `ext.streams` file-reader check in `run-os-sandboxed`.
