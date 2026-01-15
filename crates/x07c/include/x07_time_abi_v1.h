#ifndef X07_TIME_ABI_V1_H
#define X07_TIME_ABI_V1_H

// X07 Time Backend ABI (v1)
//
// This header is *pinned* and is intended to be included by:
//  - the generated C produced by x07c (call sites)
//  - the native time backend library implementation (libx07_time.a)
//
// Design goals:
//  - Stable C ABI across platforms
//  - Minimal surface: only what the tzdb builtins need
//  - Deterministic behavior (no host zoneinfo)

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} ev_bytes;

ev_bytes ev_bytes_alloc(uint32_t len);

void ev_trap(int32_t code);

enum {
  EV_TRAP_TIME_INTERNAL = 9200
};

// Return 1 if tzid exists in the pinned snapshot, else 0.
uint32_t ev_time_tzdb_is_valid_tzid_v1(ev_bytes tzid);

// Return a DurationDocV1 (X7DU) containing the UTC offset at (unix_s_lo, unix_s_hi).
ev_bytes ev_time_tzdb_offset_duration_v1(ev_bytes tzid, int32_t unix_s_lo, int32_t unix_s_hi);

// Return a pinned snapshot id string like "tzdb-2025c".
ev_bytes ev_time_tzdb_snapshot_id_v1(void);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_TIME_ABI_V1_H
