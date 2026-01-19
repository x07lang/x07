# Guide: Building CLI apps

X07 is intended to make it easy for agents to build robust command-line tools.

## Canonical approach

1. Treat runner `input` as your argument payload (for example: newline-separated fields or a small bytes encoding).
2. Optionally use the external `ext-cli` package for a declarative spec and parsing helpers.
3. Keep business logic pure so it can be tested in deterministic worlds.

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
