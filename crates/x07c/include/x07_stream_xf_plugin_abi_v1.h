#ifndef X07_STREAM_XF_PLUGIN_ABI_V1_H
#define X07_STREAM_XF_PLUGIN_ABI_V1_H

// X07 Stream Transducer Plugin ABI (v1)
//
// This header is pinned and is intended to be used by:
//  - stream xf plugin implementations (native code, static link)
//  - the X07 toolchain/runtime wrapper that calls plugins

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- Runtime hooks (provided by generated C) ---

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} ev_bytes;

ev_bytes ev_bytes_alloc(uint32_t len);
void ev_trap(int32_t code);

// --- Minimal value types (stable) ---

typedef struct {
  const uint8_t* ptr;
  uint32_t len;
} x07_bytes_view_v1;

typedef struct {
  uint8_t* ptr;
  uint32_t cap;
  uint32_t len; // plugin sets len <= cap before commit
} x07_out_buf_v1;

typedef struct {
  uint8_t* ptr;
  uint32_t cap;
  uint32_t used;
} x07_scratch_v1;

typedef struct {
  uint32_t max_out_bytes_per_step;
  uint32_t max_out_items_per_step;
  uint32_t max_out_buf_bytes;
  uint32_t max_state_bytes;
  uint32_t max_cfg_bytes;
  uint32_t max_scratch_bytes;
} x07_xf_budget_v1;

typedef struct x07_xf_emit_v1 {
  void* emit_ctx;
  int32_t (*emit_alloc)(void* emit_ctx, uint32_t cap, x07_out_buf_v1* out);
  int32_t (*emit_commit)(void* emit_ctx, const x07_out_buf_v1* out);
} x07_xf_emit_v1;

// --- Plugin descriptor (v1) ---

typedef struct x07_stream_xf_plugin_v1 {
  // ABI and identity
  uint32_t abi_tag;       // 'X7XF' as u32: 0x46584637 (little-endian)
  uint32_t abi_version;   // 1
  const char* plugin_id;  // stable id string
  uint32_t flags;         // X07_XF_FLAG_*

  // Optional stream typing (brands)
  const char* in_item_brand;
  const char* out_item_brand;

  // State + scratch sizing
  uint32_t state_size;
  uint32_t state_align;   // power of two (>=8)
  uint32_t scratch_hint;
  uint32_t scratch_max;

  // Lifecycle
  int32_t (*init)(
    void* state,
    x07_scratch_v1* scratch,
    x07_bytes_view_v1 cfg,
    x07_xf_emit_v1 emit,
    x07_xf_budget_v1 budget);

  int32_t (*step)(
    void* state,
    x07_scratch_v1* scratch,
    x07_bytes_view_v1 in,
    x07_xf_emit_v1 emit,
    x07_xf_budget_v1 budget);

  int32_t (*flush)(
    void* state,
    x07_scratch_v1* scratch,
    x07_xf_emit_v1 emit,
    x07_xf_budget_v1 budget);

  void (*drop)(void* state);
} x07_stream_xf_plugin_v1;

// Flags
#define X07_XF_FLAG_DETERMINISTIC_ONLY (1u << 0)
#define X07_XF_FLAG_NONDET_OS_ONLY     (1u << 1)
#define X07_XF_FLAG_CFG_CANON_JSON     (1u << 2)

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_STREAM_XF_PLUGIN_ABI_V1_H

