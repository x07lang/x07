#ifndef X07_EXT_RAND_ABI_V1_H
#define X07_EXT_RAND_ABI_V1_H

// X07 External RAND Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native rand backend library implementation (libx07_ext_rand.a)

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} ev_bytes;

typedef struct {
  uint32_t tag;
  union {
    ev_bytes ok;
    uint32_t err;
  } payload;
} ev_result_bytes;

// Runtime hooks required by the backend (provided by generated C).
ev_bytes ev_bytes_alloc(uint32_t len);
void ev_trap(int32_t code);

// v1 entrypoints used by os.rand.* builtins.
ev_result_bytes x07_ext_rand_bytes_v1(int32_t n, ev_bytes caps);
ev_result_bytes x07_ext_rand_u64_v1(ev_bytes caps);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_RAND_ABI_V1_H

