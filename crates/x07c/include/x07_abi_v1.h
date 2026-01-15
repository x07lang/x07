#ifndef X07_ABI_V1_H
#define X07_ABI_V1_H

#include <stdint.h>

// ABI v1 value layouts for the X07 C backend.
// Normative spec: dev-docs/x07-internal-docs/spec/abi/abi-v1.md

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} ev_bytes_v1_t;

typedef struct {
  uint8_t* ptr;
  uint32_t len;
#ifdef X07_DEBUG_BORROW
  uint64_t aid;
  uint64_t bid;
  uint32_t off_bytes;
#endif
} ev_bytes_view_v1_t;

typedef struct {
  uint8_t* data;
  uint32_t len;
  uint32_t cap;
#ifdef X07_DEBUG_BORROW
  uint64_t dbg_aid;
#endif
} ev_vec_u8_v1_t;

typedef ev_bytes_v1_t ev_box_bytes_v1_t;

typedef struct {
  uint32_t tag;
  uint32_t payload;
} ev_option_i32_v1_t;

typedef struct {
  uint32_t tag;
  ev_bytes_v1_t payload;
} ev_option_bytes_v1_t;

typedef struct {
  uint32_t tag;
  union {
    uint32_t ok;
    uint32_t err;
  } payload;
} ev_result_i32_v1_t;

typedef struct {
  uint32_t tag;
  union {
    ev_bytes_v1_t ok;
    uint32_t err;
  } payload;
} ev_result_bytes_v1_t;

typedef struct {
  uint32_t data;
  uint32_t vtable;
} ev_iface_v1_t;

typedef struct {
  void* ctx;
  void* (*alloc)(void* ctx, uint32_t size, uint32_t align);
  void* (*realloc)(void* ctx, void* ptr, uint32_t old_size, uint32_t new_size, uint32_t align);
  void (*free)(void* ctx, void* ptr, uint32_t size, uint32_t align);
} ev_allocator_v1_t;

#endif
