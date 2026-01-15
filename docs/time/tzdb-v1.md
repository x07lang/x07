# TZDB v1 (time zone database)

Module: `ext.time.tzdb`

This module provides deterministic time zone offset lookup using a pinned tzdb snapshot shipped with the X07 toolchain/package.

Design constraints:

- **Deterministic**: results must not depend on the host OS’s zoneinfo.
- **Pinned snapshot**: tzdb data is versioned and bundled (e.g. `2026a`), and the snapshot id is exposed in the API.
- **Fast**: offset lookup is O(log N) in number of transitions for the zone.

## Canonical API (v1)

- `is_valid_tzid_v1(tzid: bytes_view) -> i32`
  - Returns 1 if tzid exists in the pinned snapshot; else 0.
- `offset_duration_v1(tzid: bytes_view, unix_s_lo: i32, unix_s_hi: i32) -> bytes`
  - Returns a **Duration doc** (X7DU) representing the UTC offset at that instant.
  - On failure returns an ERR duration doc (with tzdb-specific error codes, see below).
- `snapshot_id_v1() -> bytes`
  - Returns UTF-8 like `"tzdb-<IANA release>"` (e.g. `"tzdb-2025b"`) (pinned id).

## Error codes (v1)

Returned in the ERR Duration doc’s `err_code`:

- `SPEC_ERR_TZDB_INVALID_TZID = 100`
- `SPEC_ERR_TZDB_RANGE = 101`
- `SPEC_ERR_TZDB_INTERNAL = 102`

## Implementation note (native)

The recommended implementation is a small native adapter that:
- embeds a compact precompiled tzdb table in the binary, and
- performs binary search over transition times.

Source tzdb formats:
- IANA tzdb zoneinfo files use TZif (see RFC8536).
