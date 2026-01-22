# Your first X07 program

This page shows the minimal “hello world” shape **for agents** and **for humans**.

X07 programs are stored as **x07AST JSON** (`*.x07.json`). Agents should modify programs via JSON Patch applied to a known-good base.

## Create a project (canonical)

```bash
mkdir myapp
cd myapp
x07 --init
```

This creates:

- `x07.json` (with `test`, `os`, and `sandbox` profiles)
- `x07.lock.json`
- `src/main.x07.json` (a minimal program)
- `tests/tests.json` + `tests/smoke.x07.json` (a deterministic harness smoke test)

If you are creating a publishable package, use `x07 --init --package` to also create `x07-package.json`.

## Make it return bytes

Create `patch.json`:

```json
[
  {"op":"replace","path":"/solve","value":["bytes.lit","hello\\n"]}
]
```

Apply and validate:

```bash
x07 ast apply-patch --in src/main.x07.json --patch patch.json --out src/main.x07.json --validate
x07 fmt --input src/main.x07.json --write
```

## Lint

```bash
x07 lint --input src/main.x07.json --world solve-pure
```

`x07 lint` prints an `x07diag` JSON report to stdout.

## Run (deterministic)

```bash
x07 run
```

The runner prints a JSON report; the program’s bytes output is in `solve_output_b64`.

## Next

- For packages and lockfiles, see [Packages](../packages/index.md).
- For the repair loop, see [Repair loop](../toolchain/repair-loop.md).
