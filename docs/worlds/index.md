# Worlds

A **world** is X07’s capability profile: it defines what resources a program can touch.

This is one of the most important design choices for agent reliability.

## OS worlds

Used for:

- production programs
- real disk/network/time/process access

OS worlds are always opt-in, and sandboxed variants are governed by explicit policy.

## World matrix

- `run-os`: real OS adapters (disk/net/time/env/process)
- `run-os-sandboxed`: policy-limited OS adapters

## Profiles (recommended front door)

Define `x07.json.profiles` and a `default_profile`, and use:

- `x07 run` (uses `default_profile`)
- `x07 run --profile <name>` (pick explicitly)

See:
- [OS worlds](os-worlds.md)
- [Running programs](../toolchain/running-programs.md)
