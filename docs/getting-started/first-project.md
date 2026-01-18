# Your first X07 program

This page shows the minimal “hello world” shape **for agents** and **for humans**.

X07 programs are stored as **x07AST JSON** (`*.x07.json`). Agents should modify programs via JSON Patch applied to a known-good base.

## Create an entry program

```bash
mkdir myapp
cd myapp
x07 ast init --world solve-pure --module main --kind entry --out main.x07.json
```

This writes a valid `main.x07.json` and prints a small JSON report (including `schema_version` and a sha256 of the file).

If you want a full project skeleton (with `x07.json` / `x07-package.json` / `x07.lock.json`), run `x07 --init` instead (see [Packages](../packages/index.md)).

## Make it return bytes

Create `patch.json`:

```json
[
  {"op":"replace","path":"/solve","value":["bytes.lit","hello\\n"]}
]
```

Apply and validate:

```bash
x07 ast apply-patch --in main.x07.json --patch patch.json --out main.x07.json --validate
```

## Lint

```bash
x07c lint --input main.x07.json --world solve-pure
```

`x07c lint` prints an `x07diag` JSON report to stdout.

## Run (deterministic)

```bash
x07-host-runner --program main.x07.json --world solve-pure
```

The runner prints a JSON report; the program’s bytes output is in `solve_output_b64`.

## Next

- For packages and lockfiles, see [Packages](../packages/index.md).
- For the repair loop, see [Diagnostics & repair](../toolchain/diagnostics-repair.md).
