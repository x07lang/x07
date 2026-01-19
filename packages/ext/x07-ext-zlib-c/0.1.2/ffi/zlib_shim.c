#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <zlib.h>

#define X07_EXT_ZLIB_MAX_BUFS 4096u

static uint8_t* g_bufs[X07_EXT_ZLIB_MAX_BUFS];
static uint32_t g_lens[X07_EXT_ZLIB_MAX_BUFS];

static uint32_t x07_ext_zlib_alloc_slot(void) {
    for (uint32_t i = 1; i < X07_EXT_ZLIB_MAX_BUFS; i++) {
        if (!g_bufs[i]) return i;
    }
    return 0;
}

int32_t x07_ext_zlib_compress_alloc(const uint8_t* source, uint32_t source_len, uint32_t* out_handle) {
    if (out_handle) *out_handle = 0;

    uLong bound = compressBound((uLong)source_len);
    if (bound > (uLong)UINT32_MAX) return -2;

    uint8_t* buf = (uint8_t*)malloc((size_t)bound);
    if (!buf) return -1;

    uLongf out_len = (uLongf)bound;
    int rc = compress((Bytef*)buf, &out_len, (const Bytef*)source, (uLong)source_len);
    if (rc != 0) {
        free(buf);
        return (int32_t)rc;
    }
    if (out_len > (uLongf)UINT32_MAX) {
        free(buf);
        return -2;
    }

    uint32_t h = x07_ext_zlib_alloc_slot();
    if (!h) {
        free(buf);
        return -1;
    }
    g_bufs[h] = buf;
    g_lens[h] = (uint32_t)out_len;
    if (out_handle) *out_handle = h;
    return 0;
}

int32_t x07_ext_zlib_uncompress_alloc(
    const uint8_t* source,
    uint32_t source_len,
    uint32_t max_size,
    uint32_t* out_handle
) {
    if (out_handle) *out_handle = 0;
    if (max_size > UINT32_MAX) return -2;

    uint8_t* buf = (uint8_t*)malloc((size_t)max_size);
    if (!buf) return -1;

    uLongf out_len = (uLongf)max_size;
    int rc = uncompress((Bytef*)buf, &out_len, (const Bytef*)source, (uLong)source_len);
    if (rc != 0) {
        free(buf);
        return (int32_t)rc;
    }
    if (out_len > (uLongf)UINT32_MAX) {
        free(buf);
        return -2;
    }

    uint32_t h = x07_ext_zlib_alloc_slot();
    if (!h) {
        free(buf);
        return -1;
    }
    g_bufs[h] = buf;
    g_lens[h] = (uint32_t)out_len;
    if (out_handle) *out_handle = h;
    return 0;
}

static int32_t x07_ext_zlib_inflate_alloc_window_bits(
    const uint8_t* source,
    uint32_t source_len,
    uint32_t max_size,
    int window_bits,
    uint32_t* out_handle
) {
    if (out_handle) *out_handle = 0;

    uint8_t* buf = (uint8_t*)malloc((size_t)max_size);
    if (!buf && max_size != 0) return -1;

    z_stream strm;
    memset(&strm, 0, sizeof(strm));
    strm.next_in = (Bytef*)source;
    strm.avail_in = (uInt)source_len;
    strm.next_out = (Bytef*)buf;
    strm.avail_out = (uInt)max_size;

    int rc = inflateInit2(&strm, window_bits);
    if (rc != Z_OK) {
        if (buf) free(buf);
        return (int32_t)rc;
    }

    rc = inflate(&strm, Z_FINISH);
    if (rc != Z_STREAM_END) {
        inflateEnd(&strm);
        if (buf) free(buf);
        return (int32_t)rc;
    }

    rc = inflateEnd(&strm);
    if (rc != Z_OK) {
        if (buf) free(buf);
        return (int32_t)rc;
    }

    if (strm.total_out > (uLong)UINT32_MAX) {
        if (buf) free(buf);
        return -2;
    }

    uint32_t h = x07_ext_zlib_alloc_slot();
    if (!h) {
        if (buf) free(buf);
        return -1;
    }
    g_bufs[h] = buf;
    g_lens[h] = (uint32_t)strm.total_out;
    if (out_handle) *out_handle = h;
    return 0;
}

int32_t x07_ext_zlib_gzip_decompress_alloc(
    const uint8_t* source,
    uint32_t source_len,
    uint32_t max_size,
    uint32_t* out_handle
) {
    return x07_ext_zlib_inflate_alloc_window_bits(
        source,
        source_len,
        max_size,
        16 + MAX_WBITS,
        out_handle
    );
}

int32_t x07_ext_zlib_inflate_raw_alloc(
    const uint8_t* source,
    uint32_t source_len,
    uint32_t max_size,
    uint32_t* out_handle
) {
    return x07_ext_zlib_inflate_alloc_window_bits(source, source_len, max_size, -MAX_WBITS, out_handle);
}

uint32_t x07_ext_zlib_buf_len(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_ZLIB_MAX_BUFS) return 0;
    return g_lens[handle];
}

const uint8_t* x07_ext_zlib_buf_ptr(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_ZLIB_MAX_BUFS) return (const uint8_t*)0;
    return g_bufs[handle];
}

void x07_ext_zlib_buf_free(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_ZLIB_MAX_BUFS) return;
    if (g_bufs[handle]) free(g_bufs[handle]);
    g_bufs[handle] = (uint8_t*)0;
    g_lens[handle] = 0;
}
