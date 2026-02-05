# `x07-sm-arch-contracts-smoke`

Reference implementation for **Prompt 8** in [`docs/agent/readiness-checks.md`](../../../agent/readiness-checks.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Generate: `x07 sm gen --input arch/sm/specs/lifecycle.sm.json --out gen/sm --write`
- Drift check: `x07 sm gen --input arch/sm/specs/lifecycle.sm.json --out gen/sm --check`
- Generated tests: `x07 test --manifest gen/sm/tests.manifest.json`
- App tests: `x07 test --manifest tests/tests.json`
- Run (stdin): `printf 'start\ntick\nfinish\n' | x07 run --stdin`
- Arch contracts: `x07 arch check --write-lock`
