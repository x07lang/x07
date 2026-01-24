# Testing by example (`x07 test` + `std.test`)

This page is a hands-on tutorial for the built-in deterministic test harness.

`x07 test` is designed for agents:

- a single canonical command
- stable, machine-readable JSON report output (`x07.x07test@0.2.0`)
- deterministic execution in solve worlds

Companion example (in the `x07` repo):

- `examples/tutorials/testing_by_example/`

## What `x07 test` runs (and what it does not)

`x07 test` runs tests in deterministic solve worlds:

- `solve-pure`
- `solve-fs` (read-only fixture filesystem)
- `solve-rr` (fixture request/response replay)
- `solve-kv` (fixture key/value store)
- `solve-full` (combined fixtures)

`x07 test` does **not** run OS-world integration tests (`run-os` / `run-os-sandboxed`). For OS-world smoke tests, use `x07 run` in CI and assert on the runner report.

## The three artifacts of testing

1. A test entry function (exported), typically returning `result_i32`.
2. A tests manifest (`tests/tests.json`) describing which entrypoints to run.
3. Optional fixtures (`tests/fixtures/...`) for worlds like `solve-fs` or `solve-kv`.

## Example project (pure + solve-fs)

Create a project:

```bash
mkdir testing-demo
cd testing-demo
x07 init
```

### Step 1: add world-specific test modules

World-gating is enforced at compile time. If a module calls `fs.*` anywhere, it cannot be compiled in `solve-pure`.

Keep solve-world tests in separate modules based on the capabilities they use:

- `tests/smoke.x07.json` (solve-pure)
- `tests/smoke_fs.x07.json` (solve-fs)

Replace `tests/smoke.x07.json` with:

```json
{
  "schema_version": "x07.x07ast@0.1.0",
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

Create `tests/smoke_fs.x07.json`:

```json
{
  "schema_version": "x07.x07ast@0.1.0",
  "kind": "module",
  "module_id": "smoke_fs",
  "imports": ["std.test", "std.fs"],
  "decls": [
    { "kind": "export", "names": ["smoke_fs.reads_fixture"] },
    {
      "kind": "defn",
      "name": "smoke_fs.reads_fixture",
      "params": [],
      "result": "result_i32",
      "body": [
        "begin",
        ["let", "p", ["bytes.lit", "hello.txt"]],
        ["let", "b", ["std.fs.read", ["bytes.view", "p"]]],
        ["let", "expected", ["bytes.concat", ["bytes.lit", "Hello_fixture"], ["bytes1", 10]]],
        ["try", ["std.test.assert_bytes_eq", "b", "expected", ["std.test.code_assert_bytes_eq"]]],
        ["std.test.pass"]
      ]
    }
  ]
}
```

### Step 2: add fixture data

Create:

```bash
mkdir -p tests/fixtures/fs_read
printf 'Hello_fixture\n' > tests/fixtures/fs_read/hello.txt
```

### Step 3: write the tests manifest

Replace `tests/tests.json` with:

```json
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": [
    { "id": "pure_add", "world": "solve-pure", "entry": "smoke.pure_add", "expect": "pass" },
    {
      "id": "fs_read",
      "world": "solve-fs",
      "fixture_root": "fixtures/fs_read",
      "entry": "smoke_fs.reads_fixture",
      "expect": "pass"
    }
  ]
}
```

Notes:

- `fixture_root` is resolved relative to the manifest directory (`tests/` here).
- In the test, file paths are relative to the fixture root.

### Step 4: run tests

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
