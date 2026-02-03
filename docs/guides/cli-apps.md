# Guide: Building CLI apps

X07 is intended to make it easy for agents to build robust command-line tools.

## Canonical (x07 init + x07 run)

1. Start from the CLI template (recommended):

   ```bash
   x07 init --template cli
   ```

2. Keep business logic pure and isolate I/O behind adapters so it’s easy to test.
3. Run via the single front door:

   ```bash
   # OS CLI run (argv_v1 is derived from args after --):
   x07 run -- tool --help

   # Policy-enforced run:
   x07 run --profile sandbox -- tool --help
   ```

4. Distribute a normal CLI binary (no toolchain required at runtime):

   ```bash
   x07 bundle --out dist/tool
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
- provide `argv_v1` bytes directly via `--input` (advanced).

### Canonical execution for CLI args: `x07 run -- <args...>`

If your program expects `argv_v1`, pass process args after `--` and `x07 run` will encode them into input bytes automatically:

```bash
x07 run -- tool --help
x07 run --profile sandbox -- tool --url https://example.com --depth 2 --out out/results.txt
```

This eliminates the need to manually construct `--input` bytes for normal runs. `--input` remains available for advanced usage.

See also: `examples/agent-gate/cli-ext-cli` in the `x07` repo (CI-gated reference project).

## Using `ext-cli-ux`

`ext-cli-ux` provides deterministic CLI output helpers pinned by `arch/cli/`:

- `std.cli.profile.from_arch_v1(profile_id)` (profile config)
- `std.cli.progress.render_v1(profile, done, total)` (pure progress line)
- `std.cli.table.render_v1(profile, rows_doc)` (pure table rendering)
- `std.cli.jsonl.encode_v1(doc)` (pure JSONL encoding; canonical JSON per line)

For most projects, add the canonical package via the capability map (`cli.ux`) and sync the lockfile:

```bash
# Pick NAME@VERSION from /agent/latest/catalog/capabilities.json.
x07 pkg add NAME@VERSION --sync
```

## Recommended layout

- `myapp.cli` — CLI spec + parsing
- `myapp.core` — pure logic
- `myapp.adapters` — OS I/O (fs/net/db)
- `myapp.main` — thin wiring

## Testing

- Unit tests: `x07 test --manifest tests/tests.json`
- Integration smoke tests: `x07 run --profile sandbox -- ...` in CI (assert on the runner report)

## Expert (manual inputs + troubleshooting)

- Prefer `x07 run -- <args...>` for normal runs; use `--input` only when you already have pre-encoded `argv_v1` bytes.
- If `x07 bundle` output behaves differently than `x07 run`, check whether you’re relying on an implicit OS capability that should be expressed as an explicit policy.
