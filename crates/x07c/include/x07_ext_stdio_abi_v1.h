#ifndef X07_EXT_STDIO_ABI_V1_H
#define X07_EXT_STDIO_ABI_V1_H

// X07 External STDIO Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native stdio backend library implementation (libx07_ext_stdio.a)

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

typedef struct {
  uint32_t tag;
  union {
    uint32_t ok;
    uint32_t err;
  } payload;
} ev_result_i32;

// Runtime hooks required by the backend (provided by generated C).
ev_bytes ev_bytes_alloc(uint32_t len);
void ev_trap(int32_t code);

// v1 entrypoints used by os.stdio.* builtins.
ev_result_bytes x07_ext_stdio_read_line_v1(ev_bytes caps);
ev_result_i32 x07_ext_stdio_write_stdout_v1(ev_bytes data, ev_bytes caps);
ev_result_i32 x07_ext_stdio_write_stderr_v1(ev_bytes data, ev_bytes caps);
ev_result_i32 x07_ext_stdio_flush_stdout_v1(void);
ev_result_i32 x07_ext_stdio_flush_stderr_v1(void);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_STDIO_ABI_V1_H

