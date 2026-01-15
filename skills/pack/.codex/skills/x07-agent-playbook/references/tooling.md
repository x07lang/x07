# Tooling (end-user)

This skills pack targets agents writing X07 programs using the released toolchain binaries.
It must not rely on the toolchain source repo.

## Required binaries

- `x07` (project utilities: `pkg`, `test`, `ast`, CLI specs)
- `x07c` (compiler + formatter/linter)
- `x07-host-runner` (deterministic solve-world runner)
- `x07-os-runner` (run-os / run-os-sandboxed runner, optional)

## External tools

- A C compiler available as `cc` (override with `X07_CC`).
- Optional: `clang` if using the C importer (`x07import-cli`).

## Environment variables (C backend)

- `X07_CC`: C compiler command (default: `cc`).
- `X07_CC_ARGS`: extra compiler flags (space-separated).

## Quick sanity checks

- `x07c --help`
- `x07-host-runner --help`
- `cc --version`
