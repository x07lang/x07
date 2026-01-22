# Guide: Building CLI apps

X07 is intended to make it easy for agents to build robust command-line tools.

## Canonical approach

1. Treat runner `input` as your argument payload (for example: newline-separated fields or a small bytes encoding).
2. Optionally use the external `ext-cli` package for a declarative spec and parsing helpers.
3. Keep business logic pure so it can be tested in deterministic worlds.

## Using `ext-cli`

`ext-cli` parses a declarative CLI spec plus an `argv_v1` byte encoding.

Add the package and sync the lockfile:

```bash
x07 pkg add ext-cli@0.1.3 --sync
```

`ext-cli` declares its required helper packages via `meta.requires_packages`, so `x07 pkg lock` will add and fetch them automatically.

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
