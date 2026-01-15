#ifndef X07_EXT_REGEX_ABI_V1_H
#define X07_EXT_REGEX_ABI_V1_H

// X07 External Regex Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native regex backend library implementation (libx07_ext_regex.a)

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

// v1 entrypoints used by the ext.regex native backend.
ev_bytes x07_ext_regex_compile_opts_v1(ev_bytes pat, int32_t opts_u32);
ev_bytes x07_ext_regex_exec_from_v1(ev_bytes compiled, ev_bytes text, int32_t start_i32);
ev_bytes x07_ext_regex_exec_caps_from_v1(ev_bytes compiled, ev_bytes text, int32_t start_i32);
ev_bytes x07_ext_regex_find_all_x7sl_v1(ev_bytes compiled, ev_bytes text, int32_t max_matches_i32);
ev_bytes x07_ext_regex_split_v1(ev_bytes compiled, ev_bytes text, int32_t max_parts_i32);
ev_bytes x07_ext_regex_replace_all_v1(ev_bytes compiled, ev_bytes text, ev_bytes repl, int32_t cap_limit_i32);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_REGEX_ABI_V1_H
