#include <stdint.h>

static inline int32_t ext_smoke_add_i32(int32_t a, int32_t b) { return a + b; }
static inline int32_t ext_smoke_abs_i32(int32_t x) {
  if (x < 0) {
    return -x;
  }
  return x;
}
