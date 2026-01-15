# OS time v1 (run-os*)

Module: `ext.time.os`

This module is ONLY supported in:
- `run-os`
- `run-os-sandboxed`

It is not available in deterministic `solve-*` worlds.

## Canonical API (v1)

- `now_instant_v1() -> bytes`
  - Returns an **Instant doc**, encoded identically to Duration v1 (X7DU).
  - Semantics: Unix time (`unix_s` seconds, `nanos` nanoseconds), wall-clock.
- `sleep_ms_v1(ms: i32) -> i32`
  - Sleeps for at least `ms` milliseconds.
  - Returns 1 on success, 0 on failure (policy denied / invalid arg).
- `local_tzid_v1() -> bytes`
  - Returns a result-bytes doc:
    - OK: tag=1, ver=1, tzid_len:u32, tzid_bytes (UTF-8)
    - ERR: tag=0, err_code:u32

## Policy gating (run-os-sandboxed)

The runner must enforce `schemas/run-os-policy.schema.json#/properties/time`.

Minimum required knobs:

- `time.enabled`
- `time.allow_wall_clock` (for `now_instant_v1`)
- `time.allow_sleep` (for `sleep_ms_v1`)
- `time.allow_local_tzid` (for `local_tzid_v1`)
- `time.max_sleep_ms`

The adapter must return deterministic error codes when denied.

## Error codes (v1)

- `now_instant_v1` returns an ERR Duration doc with `err_code`:
  - `300` — policy denied
  - `301` — internal error
- `local_tzid_v1` returns an ERR doc with `err_code`:
  - `1` — policy denied
  - `2` — internal error
