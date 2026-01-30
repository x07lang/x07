#ifndef X07_EXT_FS_ABI_V1_H
#define X07_EXT_FS_ABI_V1_H

// X07 External FS Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native fs backend library implementation (libx07_ext_fs.a)

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

// v1 entrypoints used by os.fs.* builtins.
ev_result_bytes x07_ext_fs_read_all_v1(ev_bytes path, ev_bytes caps);
ev_result_i32 x07_ext_fs_write_all_v1(ev_bytes path, ev_bytes data, ev_bytes caps);
ev_result_i32 x07_ext_fs_mkdirs_v1(ev_bytes path, ev_bytes caps);
ev_result_i32 x07_ext_fs_remove_file_v1(ev_bytes path, ev_bytes caps);
ev_result_i32 x07_ext_fs_remove_dir_all_v1(ev_bytes path, ev_bytes caps);
ev_result_i32 x07_ext_fs_rename_v1(ev_bytes src, ev_bytes dst, ev_bytes caps);
ev_result_bytes x07_ext_fs_list_dir_sorted_text_v1(ev_bytes path, ev_bytes caps);
ev_result_bytes x07_ext_fs_walk_glob_sorted_text_v1(ev_bytes root, ev_bytes glob, ev_bytes caps);
ev_result_bytes x07_ext_fs_stat_v1(ev_bytes path, ev_bytes caps);

// v1 streaming write handle API used by os.fs.stream_* builtins.
ev_result_i32 x07_ext_fs_stream_open_write_v1(ev_bytes path, ev_bytes caps);
ev_result_i32 x07_ext_fs_stream_write_all_v1(int32_t writer_handle, ev_bytes data);
ev_result_i32 x07_ext_fs_stream_close_v1(int32_t writer_handle);
int32_t x07_ext_fs_stream_drop_v1(int32_t writer_handle);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_FS_ABI_V1_H
