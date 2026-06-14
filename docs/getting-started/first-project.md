# Your first X07 program

This page is the minimal human “hello world”. If you are operating through a coding agent, start from [Agent quickstart](agent-quickstart.md) instead — it is the canonical on-ramp.

X07 programs are stored as **x07AST JSON** (`*.x07.json`); you edit them with JSON Patch applied to a known-good base.

## Create a project (canonical)

```bash
mkdir myapp
cd myapp
x07 init
```

This creates:

- `x07.json` (with `os` and `sandbox` profiles)
- `x07.lock.json`
- `src/main.x07.json` (a minimal program)
- `tests/tests.json` + `tests/smoke.x07.json` (a harness smoke test)

For templates (typed CLI, file I/O, JSON reporting, XTAL starters), publishable packages (`x07 init --package`), and migrating older code (`x07 migrate` / `x07 project migrate`), see [Agent quickstart](agent-quickstart.md#templates-agentic-starting-points) and [Version + compat + migration](agent-quickstart.md#05-version--compat--migration-do-this-early).

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
x07 lint --input src/main.x07.json
```

`x07 lint` prints an `x07diag` JSON report to stdout.

## Run

```bash
x07 run
```

The runner prints a JSON report; the program’s bytes output is in `solve_output_b64`.

## Next

- For packages and lockfiles, see [Packages](../packages/index.md).
- For the repair loop, see [Repair loop](../toolchain/repair-loop.md).
