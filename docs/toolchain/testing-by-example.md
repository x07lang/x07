# Testing by example (`x07 test` + `std.test`)

This page is a hands-on tutorial for the built-in test harness.

`x07 test` is designed for agents:

- a single canonical command
- stable, machine-readable JSON report output (`x07.x07test@0.2.0`)
- runs inside OS worlds (`run-os` / `run-os-sandboxed`)

Companion example (in the `x07` repo):

- `docs/examples/tutorials/testing_by_example/`

## What `x07 test` runs (and what it does not)

`x07 test` runs exported X07 functions (typically returning `result_i32`) that use `std.test` assertions.

It is not a general-purpose integration-test runner for external binaries. For full end-to-end checks, run your program via `x07 run` (or bundle a native binary via `x07 bundle`) in CI and assert on the runner report.

## The three artifacts of testing

1. A test entry function (exported), typically returning `result_i32`.
2. A tests manifest (`tests/tests.json`) describing which entrypoints to run.
3. (Optional) Test-only modules under `tests/` (kept separate from your production modules).

## Example project (single smoke test)

Create a project:

```bash
mkdir testing-demo
cd testing-demo
x07 init
```

`x07 init` creates:

- `AGENT.md` + `.agent/skills/` (agent kit)
- `tests/smoke.x07.json` with one exported test (`smoke.pass`)
- `tests/tests.json` listing that test in `run-os`

### Step 1: add a real assertion

Replace `tests/smoke.x07.json` with:

```json
{
  "schema_version": "x07.x07ast@0.3.0",
  "kind": "module",
  "module_id": "smoke",
  "imports": ["std.test"],
  "decls": [
    { "kind": "export", "names": ["smoke.pure_add"] },
    {
      "kind": "defn",
      "name": "smoke.pure_add",
      "params": [],
      "result": "result_i32",
      "body": [
        "begin",
        ["try", ["std.test.assert_i32_eq", ["+", 2, 3], 5, ["std.test.code_assert_i32_eq"]]],
        ["std.test.pass"]
      ]
    }
  ]
}
```

### Step 2: write the tests manifest

Replace `tests/tests.json` with:

```json
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": [
    { "id": "pure_add", "world": "run-os", "entry": "smoke.pure_add", "expect": "pass" }
  ]
}
```

If you want your unit tests to run under the sandboxed world, change `world` to `run-os-sandboxed` and keep tests pure (no filesystem/network I/O) unless you also provide a policy-driven integration harness.

### Step 3: run tests

```bash
x07 test --manifest tests/tests.json
```

## Interpreting failures (agent recovery loop)

`x07 test` prints a JSON report with:

- `summary` (counts + duration)
- `tests[]` entries with per-test `status`, plus optional `compile` / `run` sections

List failing test IDs:

```bash
x07 test --manifest tests/tests.json \
  | python3 - <<'PY'
import sys, json
r = json.load(sys.stdin)
for t in r.get("tests", []):
    if t.get("status") in ("fail", "error", "xfail_pass"):
        print(t.get("id"))
PY
```

See also: [Testing reference](testing.md).
