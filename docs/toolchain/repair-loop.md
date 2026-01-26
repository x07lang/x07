# Repair loop (canonical agent workflow)

X07 tooling is designed so agents can converge on a correct program by iterating over structured outputs (diagnostics, quickfixes, JSON Patch) in a deterministic world.

Prefer running the repair loop in `solve-pure` (or another `solve-*` world) so failures are reproducible.

## Automatic repair (recommended)

`x07 run`, `x07 build`, and `x07 bundle` run the repair loop automatically by default.

Control it with:

- `--repair=write` (default): write repairs back to source files
- `--repair=memory`: stage repaired copies under `.x07/repair/_staged/`
- `--repair=off`: disable auto-repair
- `--repair-max-iters N`: bound iterations (default: 3)

Use the manual loop below when you need explicit control (or when you’re working with raw `*.x07.json` files outside a project wrapper).

## The loop (single file)

### 1) Format (canonicalize)

```bash
x07 fmt --input src/main.x07.json --write --report-json
```

### 2) Lint (collect diagnostics)

```bash
x07 lint --input src/main.x07.json --world solve-pure --report-json
```

If you want the raw diagnostics report (instead of the tool wrapper), omit `--report-json`.

### 3) Apply quickfixes (when available)

```bash
x07 fix --input src/main.x07.json --world solve-pure --write --report-json
```

Notes:

- `x07 fix` applies any `diagnostic.quickfix` entries that are `kind: "json_patch"` (RFC 6902) and then re-lints the result.
- `--report-json` requires `--write` so stdout can stay JSON.

### 4) Apply an explicit patch (when required)

When `x07 fix` cannot resolve an issue (or you need a targeted semantic change), apply an explicit JSON Patch:

```bash
x07 ast apply-patch --in src/main.x07.json --patch patch.json --out src/main.x07.json --validate
```

### 5) Re-format and re-lint

```bash
x07 fmt --input src/main.x07.json --write --report-json
x07 lint --input src/main.x07.json --world solve-pure --report-json
```

Repeat until lint is green.

## Validate at the project level

After the single-file loop is clean, run the deterministic test harness:

```bash
x07 test --manifest tests/tests.json
```

If you changed dependencies, refresh the lockfile:

```bash
x07 pkg lock --project x07.json
```

## Output contracts (for agents)

- Diagnostics schema: `spec/x07diag.schema.json`
- Tool wrapper schema (`--report-json`): `spec/x07c.report.schema.json` (`schema_version: "x07c.report@0.1.0"`)
