#ifndef X07_EXT_DB_MYSQL_ABI_V1_H
#define X07_EXT_DB_MYSQL_ABI_V1_H

// X07 External DB MySQL Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native mysql backend library implementation (libx07_ext_db_mysql.a)

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

// v1 entrypoints used by os.db.mysql.* builtins.
ev_bytes x07_ext_db_mysql_open_v1(ev_bytes req, ev_bytes caps);
ev_bytes x07_ext_db_mysql_query_v1(ev_bytes req, ev_bytes caps);
ev_bytes x07_ext_db_mysql_exec_v1(ev_bytes req, ev_bytes caps);
ev_bytes x07_ext_db_mysql_close_v1(ev_bytes req, ev_bytes caps);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_DB_MYSQL_ABI_V1_H

