#ifndef X07_EXT_JSONSCHEMA_ABI_V1_H
#define X07_EXT_JSONSCHEMA_ABI_V1_H

// X07 External JSON Schema Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native jsonschema backend library implementation (libx07_ext_jsonschema.a)

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

// v1 entrypoints used by jsonschema.* builtins.
ev_bytes x07_ext_jsonschema_compile_v1(ev_bytes schema_json);
ev_bytes x07_ext_jsonschema_validate_v1(ev_bytes compiled, ev_bytes instance_json);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_JSONSCHEMA_ABI_V1_H
