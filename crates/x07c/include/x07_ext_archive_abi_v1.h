#ifndef X07_EXT_ARCHIVE_ABI_V1_H
#define X07_EXT_ARCHIVE_ABI_V1_H

// X07 External ARCHIVE Backend ABI (v1)
//
// This header is pinned and must remain backward compatible within v1.
// It is intended to be used by:
//  - the generated C produced by x07c (call sites)
//  - the native archive backend library implementation (libx07_ext_archive.a)

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

// v1 entrypoints used by os.archive.* builtins.
ev_bytes x07_ext_archive_tar_extract_to_fs_v1(
    ev_bytes out_root,
    ev_bytes tar_path,
    ev_bytes caps_read,
    ev_bytes caps_write,
    ev_bytes profile_id
);

ev_bytes x07_ext_archive_tgz_extract_to_fs_v1(
    ev_bytes out_root,
    ev_bytes tgz_path,
    ev_bytes caps_read,
    ev_bytes caps_write,
    ev_bytes profile_id
);

ev_bytes x07_ext_archive_zip_extract_to_fs_v1(
    ev_bytes out_root,
    ev_bytes zip_path,
    ev_bytes caps_read,
    ev_bytes caps_write,
    ev_bytes profile_id
);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_EXT_ARCHIVE_ABI_V1_H

