# Time v1 (ext.time.*)

This document pins the **v1** contracts for X07’s external time package.

Design goals:

- **Single canonical way** for all common time tasks (no ad-hoc encodings).
- **LLM-first**: agents should not hand-slice offsets or invent binary layouts; use the provided helpers.
- **Deterministic by default**: pure modules are usable in `solve-*` worlds; OS time is `run-os*` only.

## Modules (package `x07:ext-time-rs`)

Pure / deterministic:

- `ext.time.duration` — duration encoding + arithmetic (DurationDocV1).
- `ext.time.rfc3339` — RFC3339 parse/format (Rfc3339DocV1).
- `ext.time.civil` — civil calendar conversions (CivilDocV1).
- `ext.time.tzdb` — deterministic tzdb snapshot + offset lookup.
- `ext.time.instant` — “absolute time” helpers using the same Unix seconds + nanos representation.

Run‑OS only (non-deterministic, policy gated):

- `ext.time.os` — OS adapters (now/sleep/local_tzid).

## World gating rule

Any API that consults the host OS (wall clock, local timezone, etc.) MUST live under `ext.time.os` and is only available in `run-os` / `run-os-sandboxed`.

## Pinned specs

- `docs/time/duration-v1.md`
- `docs/time/rfc3339-v1.md`
- `docs/time/civil-v1.md`
- `docs/time/tzdb-v1.md`
- `docs/time/os-time-v1.md`
