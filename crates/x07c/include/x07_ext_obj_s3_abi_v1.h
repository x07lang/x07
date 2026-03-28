#ifndef X07_EXT_OBJ_S3_ABI_V1_H
#define X07_EXT_OBJ_S3_ABI_V1_H

// X07 External Object S3 Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c
//  - the native S3 backend library implementation (libx07_ext_obj_s3.a)

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} ev_bytes;

// Runtime hooks required by the backend (provided by generated C).
ev_bytes ev_bytes_alloc(uint32_t len);
void ev_trap(int32_t code);

// v1 entrypoint used by os.obj.s3.dispatch_v1.
ev_bytes x07_obj_s3_dispatch_v1(ev_bytes req, ev_bytes caps);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_OBJ_S3_ABI_V1_H
