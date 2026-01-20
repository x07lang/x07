# Guide: Building CLI apps

X07 is intended to make it easy for agents to build robust command-line tools.

## Canonical approach

1. Treat runner `input` as your argument payload (for example: newline-separated fields or a small bytes encoding).
2. Optionally use the external `ext-cli` package for a declarative spec and parsing helpers.
3. Keep business logic pure so it can be tested in deterministic worlds.

## Using `ext-cli`

`ext-cli` parses a declarative CLI spec plus an `argv_v1` byte encoding.

Add packages (X07 does not resolve transitive deps yet):

```bash
x07 pkg add ext-cli@<version>
x07 pkg add ext-json-rs@<version>
x07 pkg add ext-data-model@<version>
x07 pkg lock
```

`argv_v1` encoding:

- `u32_le(argc)`
- then `argc` tokens of:
  - `u32_le(len)`
  - `len` raw bytes

When running under `x07 run` (or the underlying runners), you still pass `argv_v1` bytes via `--input`.

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
