# Guide: Building CLI apps

X07 is intended to make it easy for agents to build robust command-line tools.

## Canonical approach

1. Define your CLI spec using `std.cli` (or an external CLI spec package).
2. Parse `argv` via the OS adapter (in `run-os` / `run-os-sandboxed`).
3. Keep business logic pure so it can be tested in fixture worlds.

## Recommended layout

- `myapp.cli` — CLI spec + parsing
- `myapp.core` — pure logic
- `myapp.adapters` — OS I/O (fs/net/db)
- `myapp.main` — thin wiring

## Deterministic tests

Even for CLI apps, keep tests deterministic:

- feed mocked argv bytes
- run in `solve-pure` or fixture worlds
- assert exact output bytes

Then separately smoke-test in OS world.
