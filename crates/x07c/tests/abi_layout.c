#include "x07_abi_v2.h"

#include <stdalign.h>

#define EV_STATIC_ASSERT(COND, MSG) _Static_assert((COND), MSG)

EV_STATIC_ASSERT(sizeof(ev_bytes_v2_t) >= sizeof(void*) + sizeof(uint32_t), "bytes size");
EV_STATIC_ASSERT(alignof(ev_bytes_v2_t) == alignof(void*), "bytes alignment");

EV_STATIC_ASSERT(sizeof(ev_bytes_view_v2_t) >= sizeof(void*) + sizeof(uint32_t), "bytes_view size");
EV_STATIC_ASSERT(alignof(ev_bytes_view_v2_t) == alignof(void*), "bytes_view alignment");

EV_STATIC_ASSERT(sizeof(ev_vec_u8_v2_t) >= sizeof(void*) + 2u * sizeof(uint32_t), "vec_u8 size");
EV_STATIC_ASSERT(alignof(ev_vec_u8_v2_t) == alignof(void*), "vec_u8 alignment");

EV_STATIC_ASSERT(sizeof(ev_option_i32_v2_t) >= 2u * sizeof(uint32_t), "option_i32 size");
EV_STATIC_ASSERT(alignof(ev_option_i32_v2_t) == alignof(uint32_t), "option_i32 alignment");

EV_STATIC_ASSERT(sizeof(ev_option_bytes_v2_t) >= sizeof(uint32_t) + sizeof(ev_bytes_v2_t), "option_bytes size");
EV_STATIC_ASSERT(alignof(ev_option_bytes_v2_t) == alignof(ev_bytes_v2_t), "option_bytes alignment");

EV_STATIC_ASSERT(sizeof(ev_result_i32_v2_t) >= 2u * sizeof(uint32_t), "result_i32 size");
EV_STATIC_ASSERT(alignof(ev_result_i32_v2_t) == alignof(uint32_t), "result_i32 alignment");

EV_STATIC_ASSERT(sizeof(ev_result_bytes_v2_t) >= sizeof(uint32_t) + sizeof(ev_bytes_v2_t), "result_bytes size");
EV_STATIC_ASSERT(alignof(ev_result_bytes_v2_t) == alignof(ev_bytes_v2_t), "result_bytes alignment");

EV_STATIC_ASSERT(sizeof(ev_iface_v2_t) == 2u * sizeof(uint32_t), "iface size");
EV_STATIC_ASSERT(alignof(ev_iface_v2_t) == alignof(uint32_t), "iface alignment");

EV_STATIC_ASSERT(sizeof(ev_allocator_v1_t) >= 4u * sizeof(void*), "allocator size");
EV_STATIC_ASSERT(alignof(ev_allocator_v1_t) == alignof(void*), "allocator alignment");
