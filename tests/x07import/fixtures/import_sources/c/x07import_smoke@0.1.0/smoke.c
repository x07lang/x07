#include <stdint.h>

static inline int32_t add1(int32_t x) { return x + 1; }

static inline int32_t abs_i32(int32_t x) {
  if (x < 0) {
    return -x;
  }
  return x;
}
