# Tooling (end-user)

This skills pack targets agents writing X07 programs using the released toolchain binaries.
It must not rely on the toolchain source repo.

## Required binaries

- `x07` (canonical front door: `run`, `fmt`, `lint`, `fix`, plus `pkg`, `test`, `ast`, CLI specs, `guide`)
- `x07-os-runner` (execution backend for OS worlds)

## Optional (advanced) binaries

- `x07c` (compiler driver; exposed for expert usage)

## External tools

- A C compiler available as `cc` (override with `X07_CC`).
- Optional: `clang` if using the C importer (`x07import-cli`).

## Environment variables (C backend)

- `X07_CC`: C compiler command (default: `cc`).
- `X07_CC_ARGS`: extra compiler flags (space-separated).

## Quick sanity checks

- `x07 --help`
- `x07 run --help`
- `x07 guide | head`
- `cc --version`

Expert-only:

- `x07-os-runner --help`
