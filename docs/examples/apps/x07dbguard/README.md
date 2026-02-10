# x07dbguard

Reference implementation for **Roadmap App 3** in [`docs/agent/apps.md`](../../../agent/apps.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Run: `x07 run --stdin` (see commands below)
- Bundle: `x07 bundle --profile sandbox --out dist/x07dbguard`

## Commands

### plan
Reads migration plan and prints canonical JSON with fingerprint.
```bash
echo -n "plan" | x07 run --stdin
```

### verify
Verifies schema fingerprint matches expected value.
```bash
echo -n "verify" | x07 run --stdin
```

### apply
Applies migrations to a SQLite database (run-os-sandboxed only).
```bash
mkdir -p out
echo -n "apply out/dbguard.sqlite" | x07 run --profile sandbox --stdin
```
The sandbox policy allows writing only under `out/` by default.

## Running Tests

Unit + RR tests:
```bash
x07 test --manifest tests/tests.json
```

All tests including PBT:
```bash
x07 test --all --manifest tests/tests.json
```

Deterministic repeat:
```bash
x07 test --all --manifest tests/tests.json --repeat 2
```

## CI Telemetry

Published artifacts are written under `target/` after running:
```bash
bash ci/telemetry.sh                 # fmt/lint + x07test JSON
bash ci/trust.sh                     # trust report (trust.json/trust.html)
bash ci/review_diff.sh <FROM> <TO>   # semantic diff (diff.html/diff.json)
```

## Project Structure

```
x07.json                    # project manifest
x07.lock.json               # lockfile
src/
  main.x07.json             # entry point
  app.x07.json              # CLI dispatch
  app/
    plan.x07.json           # plan command (with contracts)
    apply.x07.json          # apply command
    verify.x07.json         # verify command
    fingerprint.x07.json    # drift fingerprinting (with contracts)
    util.x07.json           # generic stable_unique + concrete u32 version
arch/
  manifest.x07arch.json     # architecture manifest
  db/                        # migration plans
  rr/                        # record/replay policies
  budgets/                   # budget profiles (db_plan_v1, db_apply_v1, db_verify_v1)
tests/
  tests.json                # test manifest
  unit.x07.json             # unit tests (solve-pure)
  rr_verify.x07.json        # RR tests (solve-rr)
  pbt_fingerprint.x07.json  # PBT test (solve-pure)
  fixtures/replay/rr/       # RR cassette fixtures
ci/
  telemetry.sh              # fmt/lint/fix/test/pbt reports
  trust.sh                  # trust gates
  review_diff.sh            # semantic diff
```
