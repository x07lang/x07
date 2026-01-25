# Guide: Building CLI apps

X07 is intended to make it easy for agents to build robust command-line tools.

## Canonical (x07 init + x07 run)

1. Start from the CLI template (recommended):

   ```bash
   x07 init --template cli
   ```

2. Keep business logic pure so it can be tested in deterministic worlds (`solve-*`).
3. Run via the single front door:

   ```bash
   # Deterministic default (solve-* via x07.json profiles):
   x07 run

   # OS CLI run (argv_v1 is derived from args after --):
   x07 run --profile os -- tool --help
   ```

4. Distribute a normal CLI binary (no toolchain required at runtime):

   ```bash
   x07 bundle --profile os --out dist/tool
   ./dist/tool --help
   ```

   For policy-enforced distribution:

   ```bash
   x07 bundle --profile sandbox --out dist/tool
   ```

## Using `ext-cli`

`ext-cli` parses a declarative CLI spec plus an `argv_v1` byte encoding.

If you didn’t start from `x07 init --template cli`, add the canonical CLI packages from the capability map (see [Agent contracts](../agent/contract.md)) and then sync the lockfile:

```bash
# Pick NAME@VERSION from /agent/latest/catalog/capabilities.json.
x07 pkg add NAME@VERSION --sync
```

`argv_v1` encoding:

- `u32_le(argc)`
- then `argc` tokens of:
  - `u32_le(len)`
  - `len` raw bytes

When running under `x07 run`, you can either:

- pass process args after `--` (recommended; `x07 run` encodes `argv_v1` into input bytes), or
- provide `argv_v1` bytes directly via `--input` (advanced; useful for fixtures).

### Canonical execution for CLI args: `x07 run -- <args...>`

If your program expects `argv_v1`, pass process args after `--` and `x07 run` will encode them into input bytes automatically:

```bash
x07 run --profile test -- tool --help
x07 run --profile os -- tool --url https://example.com --depth 2 --out out/results.txt
```

This eliminates the need to manually construct `--input` bytes for normal runs. `--input` remains available for fixture-based tests and advanced usage.

See also: `examples/agent-gate/cli-ext-cli` in the `x07` repo (CI-gated reference project).

## Recommended layout

- `myapp.cli` — CLI spec + parsing
- `myapp.core` — pure logic
- `myapp.adapters` — OS I/O (fs/net/db)
- `myapp.main` — thin wiring

## Deterministic tests

Even for CLI apps, keep tests deterministic:

- feed mocked input bytes
- run in `solve-pure` or fixture worlds
- assert exact output bytes

Then separately smoke-test in OS world.

## Expert appendix

- Raw input bytes (fixtures): `x07 run --profile test --input input.bin`
- Debug runner behavior directly (advanced):
  - solve worlds: `x07-host-runner --project x07.json`
  - OS worlds: `x07-os-runner --project x07.json --world run-os`
