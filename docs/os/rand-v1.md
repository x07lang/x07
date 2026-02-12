# Rand v1 (run-os) — External CSPRNG Package

This document pins the **v1 contract** for `ext-rand` (OS-backed cryptographically secure random bytes) for OS-world X07 programs.

---

## Worlds

This package is available only in:

- `run-os`
- `run-os-sandboxed`

It must not be usable in deterministic `solve-*` worlds.

---

## Caps encoding: `RandCapsV1`

`RandCapsV1` is a **fixed-length 12-byte** struct.

All integers are **u32_le**.

| Offset | Size | Field |
|---:|---:|---|
| 0  | 4 | `version` (must be `1`) |
| 4  | 4 | `max_bytes_per_call` |
| 8  | 4 | `flags` (reserved; must be `0` in v1) |

Effective limits:

- `effective_max = min(policy_max, caps_max)` for non‑zero caps values.
- If `max_bytes_per_call` is `0`, it means “use policy/default”.

---

## Public API (package module surface)

Package: `packages/ext/x07-ext-rand/0.1.0/`

Modules:

- `packages/ext/x07-ext-rand/0.1.0/modules/std/os/rand.x07.json` (`std.os.rand`)
- `packages/ext/x07-ext-rand/0.1.0/modules/std/os/rand/spec.x07.json` (`std.os.rand.spec`)

### Caps helpers (`std.os.rand.spec`)

- `std.os.rand.spec.caps_default_v1() -> bytes`
- `std.os.rand.spec.caps_pack_v1(max_bytes_per_call, flags) -> bytes`
- `std.os.rand.spec.caps_*_v1(caps: bytes_view) -> i32` accessors

### Random operations (`std.os.rand`)

All operations take `caps: bytes` (`RandCapsV1`).

- `std.os.rand.bytes_v1(n: i32, caps: bytes) -> result_bytes`
  - Returns exactly `n` random bytes.
  - Errors if `n < 0` or `n > max_bytes_per_call`.
- `std.os.rand.u64_v1(caps: bytes) -> result_bytes`
  - Returns 8 random bytes.

### Determinism note

This package is intentionally **nondeterministic** and must use a real OS CSPRNG. It must fail closed (no deterministic fallback).

---

## Builtins (runner/compiler integration)

The module functions above call these builtins (OS-world only):

- `os.rand.bytes_v1(n, caps) -> result_bytes`
- `os.rand.u64_v1(caps) -> result_bytes`

In this repo, these builtins call the native backend:

- Rust staticlib: `crates/x07-ext-rand-native/`
- ABI header: `crates/x07c/include/x07_ext_rand_abi_v1.h`
- Build+stage script: `scripts/build_ext_rand.sh` (stages into `deps/x07/`)

---

## Error codes (deterministic, v1)

| Code | Name |
|---:|---|
| 60201 | `RAND_ERR_DISABLED` |
| 60202 | `RAND_ERR_POLICY_DENY` |
| 60204 | `RAND_ERR_BAD_CAPS` |
| 60205 | `RAND_ERR_BAD_ARG` |
| 60215 | `RAND_ERR_IO` |

