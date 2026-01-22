# Worlds

A **world** is X07’s capability profile: it defines what resources a program can touch.

This is one of the most important design choices for agent reliability.

## Two world families

### 1) Fixture worlds (deterministic)

Used for:

- unit tests
- deterministic replay
- benchmarking
- automated repair loops

Capabilities are backed by fixtures:

- filesystem trees
- request/response cassettes
- seeded key/value stores
- virtual time

### 2) OS worlds (nondeterministic)

Used for:

- production programs
- real disk/network/time/process access

OS worlds are always opt-in, and sandboxed variants are governed by explicit policy.

## World matrix

- `solve-pure`: pure bytes → bytes, no I/O
- `solve-fs`: deterministic read-only fixture FS
- `solve-rr`: deterministic fixture-backed request/response
- `solve-kv`: deterministic seeded KV store
- `solve-full`: combination of deterministic fixture capabilities
- `run-os`: real OS adapters (disk/net/time/env/process)
- `run-os-sandboxed`: policy-limited OS adapters

## Profiles (recommended front door)

Most projects should not hardcode an OS world in `x07.json.world` (that field stays deterministic).
Instead, define `x07.json.profiles` and a `default_profile`, and use:

- `x07 run` (uses `default_profile`)
- `x07 run --profile <name>` (pick explicitly)

See:
- [Fixture worlds](fixture-worlds.md)
- [OS worlds](os-worlds.md)
- [Running programs](../toolchain/running-programs.md)
