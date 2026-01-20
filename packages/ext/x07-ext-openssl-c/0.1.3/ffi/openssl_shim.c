#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

uint8_t* SHA256(const uint8_t* data, size_t len, uint8_t* md);
uint8_t* SHA512(const uint8_t* data, size_t len, uint8_t* md);

int RAND_bytes(uint8_t* buf, int num);

typedef struct evp_pkey_st EVP_PKEY;
typedef struct evp_md_ctx_st EVP_MD_CTX;

int OBJ_txt2nid(const char* s);
EVP_PKEY* EVP_PKEY_new_raw_public_key(int type, void* e, const uint8_t* key, size_t keylen);
void EVP_PKEY_free(EVP_PKEY* pkey);
EVP_MD_CTX* EVP_MD_CTX_new(void);
void EVP_MD_CTX_free(EVP_MD_CTX* ctx);
int EVP_DigestVerifyInit(EVP_MD_CTX* ctx, void** pctx, const void* type, void* e, EVP_PKEY* pkey);
int EVP_DigestVerify(EVP_MD_CTX* ctx, const uint8_t* sig, size_t siglen, const uint8_t* tbs, size_t tbslen);

uint8_t* HMAC(
    const void* evp_md,
    const void* key,
    int key_len,
    const uint8_t* data,
    size_t data_len,
    uint8_t* md,
    unsigned int* md_len
);

uint8_t* x07_ext_openssl_sha256(const uint8_t* data, uint32_t len, uint8_t* md) {
    return SHA256(data, (size_t)len, md);
}

uint8_t* x07_ext_openssl_sha512(const uint8_t* data, uint32_t len, uint8_t* md) {
    return SHA512(data, (size_t)len, md);
}

uint8_t* x07_ext_openssl_hmac(
    const void* evp_md,
    const void* key,
    uint32_t key_len,
    const uint8_t* data,
    uint32_t data_len,
    uint8_t* md,
    uint32_t* md_len
) {
    unsigned int out_len = 0;
    uint8_t* out = HMAC(
        evp_md,
        key,
        (int)key_len,
        data,
        (size_t)data_len,
        md,
        &out_len
    );
    if (md_len) *md_len = (uint32_t)out_len;
    return out;
}

#define X07_EXT_OPENSSL_MAX_BUFS 4096u

static uint8_t* g_bufs[X07_EXT_OPENSSL_MAX_BUFS];
static uint32_t g_lens[X07_EXT_OPENSSL_MAX_BUFS];

static uint32_t x07_ext_openssl_alloc_buf_slot(void) {
    for (uint32_t i = 1; i < X07_EXT_OPENSSL_MAX_BUFS; i++) {
        if (!g_bufs[i]) return i;
    }
    return 0;
}

uint32_t x07_ext_openssl_buf_len(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_OPENSSL_MAX_BUFS) return 0;
    return g_lens[handle];
}

const uint8_t* x07_ext_openssl_buf_ptr(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_OPENSSL_MAX_BUFS) return (const uint8_t*)0;
    return g_bufs[handle];
}

void x07_ext_openssl_buf_free(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_OPENSSL_MAX_BUFS) return;
    if (g_bufs[handle]) free(g_bufs[handle]);
    g_bufs[handle] = (uint8_t*)0;
    g_lens[handle] = 0;
}

static void x07_ext_write_u32_le(uint8_t* dst, uint32_t v) {
    dst[0] = (uint8_t)(v & 0xff);
    dst[1] = (uint8_t)((v >> 8) & 0xff);
    dst[2] = (uint8_t)((v >> 16) & 0xff);
    dst[3] = (uint8_t)((v >> 24) & 0xff);
}

static uint8_t* x07_ext_make_err_doc(uint32_t code, uint32_t* out_len) {
    uint8_t* buf = (uint8_t*)malloc(9);
    if (!buf) return (uint8_t*)0;
    buf[0] = 0;
    x07_ext_write_u32_le(buf + 1, code);
    x07_ext_write_u32_le(buf + 5, 0);
    if (out_len) *out_len = 9;
    return buf;
}

int32_t x07_ext_openssl_rand_bytes_alloc(uint32_t len, uint32_t* out_handle) {
    if (out_handle) *out_handle = 0;

    uint32_t doc_len = 0;

    if (len > 1024u * 1024u) {
        uint8_t* doc = x07_ext_make_err_doc(3, &doc_len);
        if (!doc) return -1;
        uint32_t slot = x07_ext_openssl_alloc_buf_slot();
        if (!slot) {
            free(doc);
            return -1;
        }
        g_bufs[slot] = doc;
        g_lens[slot] = doc_len;
        if (out_handle) *out_handle = slot;
        return 0;
    }

    uint32_t need = 1u + len;
    uint8_t* doc = (uint8_t*)malloc((size_t)need);
    if (!doc) return -1;
    doc[0] = 1;
    if (len != 0) {
        if (RAND_bytes(doc + 1, (int)len) != 1) {
            free(doc);
            doc = x07_ext_make_err_doc(1, &doc_len);
            if (!doc) return -1;
            need = doc_len;
        }
    }

    uint32_t slot = x07_ext_openssl_alloc_buf_slot();
    if (!slot) {
        free(doc);
        return -1;
    }
    g_bufs[slot] = doc;
    g_lens[slot] = need;
    if (out_handle) *out_handle = slot;
    return 0;
}

int32_t x07_ext_openssl_rand_bytes(uint8_t* out, uint32_t len) {
    if (len == 0) return 1;
    if (!out) return 0;
    if (len > 2147483647u) return 0;
    return RAND_bytes(out, (int)len) == 1 ? 1 : 0;
}

int32_t x07_ext_openssl_ed25519_verify(
    const uint8_t* pk,
    uint32_t pk_len,
    const uint8_t* msg,
    uint32_t msg_len,
    const uint8_t* sig,
    uint32_t sig_len
) {
    if (!pk || !sig) return 0;
    if (pk_len != 32u || sig_len != 64u) return 0;

    static const uint8_t zero = 0;
    if (!msg && msg_len == 0) msg = &zero;

    int nid = OBJ_txt2nid("ED25519");
    if (nid <= 0) return 0;

    EVP_PKEY* pkey = EVP_PKEY_new_raw_public_key(nid, NULL, pk, (size_t)pk_len);
    if (!pkey) return 0;

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_DigestVerifyInit(ctx, NULL, NULL, NULL, pkey) != 1) {
        EVP_MD_CTX_free(ctx);
        EVP_PKEY_free(pkey);
        return 0;
    }

    int ok = EVP_DigestVerify(ctx, sig, (size_t)sig_len, msg, (size_t)msg_len) == 1 ? 1 : 0;
    EVP_MD_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    return ok;
}
