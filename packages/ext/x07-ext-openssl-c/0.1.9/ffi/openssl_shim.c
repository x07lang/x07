#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

uint8_t* SHA256(const uint8_t* data, size_t len, uint8_t* md);
uint8_t* SHA512(const uint8_t* data, size_t len, uint8_t* md);

const void* EVP_sha256(void);

int RAND_bytes(uint8_t* buf, int num);

typedef struct evp_pkey_st EVP_PKEY;
typedef struct evp_md_ctx_st EVP_MD_CTX;
typedef struct bignum_st BIGNUM;
typedef struct rsa_st RSA;
typedef struct ec_key_st EC_KEY;
typedef struct ecdsa_sig_st ECDSA_SIG;

int OBJ_txt2nid(const char* s);
EVP_PKEY* EVP_PKEY_new_raw_public_key(int type, void* e, const uint8_t* key, size_t keylen);
EVP_PKEY* EVP_PKEY_new_raw_private_key(int type, void* e, const uint8_t* key, size_t keylen);
int EVP_PKEY_get_raw_public_key(const EVP_PKEY* pkey, uint8_t* pub, size_t* len);
EVP_PKEY* EVP_PKEY_new(void);
void EVP_PKEY_free(EVP_PKEY* pkey);
int EVP_PKEY_set1_RSA(EVP_PKEY* pkey, RSA* key);
int EVP_PKEY_set1_EC_KEY(EVP_PKEY* pkey, EC_KEY* key);
EVP_MD_CTX* EVP_MD_CTX_new(void);
void EVP_MD_CTX_free(EVP_MD_CTX* ctx);
int EVP_DigestVerifyInit(EVP_MD_CTX* ctx, void** pctx, const void* type, void* e, EVP_PKEY* pkey);
int EVP_DigestVerify(EVP_MD_CTX* ctx, const uint8_t* sig, size_t siglen, const uint8_t* tbs, size_t tbslen);
int EVP_DigestSignInit(EVP_MD_CTX* ctx, void** pctx, const void* type, void* e, EVP_PKEY* pkey);
int EVP_DigestSign(EVP_MD_CTX* ctx, uint8_t* sigret, size_t* siglen, const uint8_t* tbs, size_t tbslen);

BIGNUM* BN_bin2bn(const uint8_t* s, int len, BIGNUM* ret);
int BN_num_bits(const BIGNUM* a);
int BN_bn2bin(const BIGNUM* a, uint8_t* to);
BIGNUM* BN_new(void);
int BN_set_word(BIGNUM* a, unsigned long w);
void BN_free(BIGNUM* a);

RSA* RSA_new(void);
void RSA_free(RSA* rsa);
int RSA_set0_key(RSA* r, BIGNUM* n, BIGNUM* e, BIGNUM* d);
int RSA_generate_key_ex(RSA* rsa, int bits, BIGNUM* e, void* cb);
void RSA_get0_key(const RSA* r, const BIGNUM** n, const BIGNUM** e, const BIGNUM** d);
void RSA_get0_factors(const RSA* r, const BIGNUM** p, const BIGNUM** q);
void RSA_get0_crt_params(
    const RSA* r,
    const BIGNUM** dmp1,
    const BIGNUM** dmq1,
    const BIGNUM** iqmp
);

EC_KEY* EC_KEY_new_by_curve_name(int nid);
void EC_KEY_free(EC_KEY* key);
int EC_KEY_set_public_key_affine_coordinates(EC_KEY* key, BIGNUM* x, BIGNUM* y);
int EC_KEY_check_key(const EC_KEY* key);

ECDSA_SIG* ECDSA_SIG_new(void);
void ECDSA_SIG_free(ECDSA_SIG* sig);
int ECDSA_SIG_set0(ECDSA_SIG* sig, BIGNUM* r, BIGNUM* s);
int i2d_ECDSA_SIG(const ECDSA_SIG* sig, uint8_t** pp);

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

static char x07_ext_b64url_char(uint8_t v) {
    static const char tbl[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    return tbl[v & 0x3fu];
}

static char* x07_ext_b64url_nopad(const uint8_t* data, size_t len, size_t* out_len) {
    if (!data && len != 0u) return (char*)0;

    size_t cap = ((len + 2u) / 3u) * 4u;
    char* out = (char*)malloc(cap + 1u);
    if (!out) return (char*)0;

    size_t i = 0u;
    size_t j = 0u;
    while (i + 3u <= len) {
        uint32_t v = ((uint32_t)data[i] << 16) | ((uint32_t)data[i + 1u] << 8) | (uint32_t)data[i + 2u];
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 18) & 0x3f));
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 12) & 0x3f));
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 6) & 0x3f));
        out[j++] = x07_ext_b64url_char((uint8_t)(v & 0x3f));
        i += 3u;
    }

    size_t rem = len - i;
    if (rem == 1u) {
        uint32_t v = ((uint32_t)data[i] << 16);
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 18) & 0x3f));
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 12) & 0x3f));
    } else if (rem == 2u) {
        uint32_t v = ((uint32_t)data[i] << 16) | ((uint32_t)data[i + 1u] << 8);
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 18) & 0x3f));
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 12) & 0x3f));
        out[j++] = x07_ext_b64url_char((uint8_t)((v >> 6) & 0x3f));
    }

    out[j] = '\0';
    if (out_len) *out_len = j;
    return out;
}

static char* x07_ext_bn_b64url(const BIGNUM* bn, size_t* out_len) {
    if (!bn) return (char*)0;
    int bits = BN_num_bits(bn);
    int n = bits <= 0 ? 1 : ((bits + 7) / 8);
    if (n <= 0) return (char*)0;
    uint8_t* raw = (uint8_t*)malloc((size_t)n);
    if (!raw) return (char*)0;
    if (bits <= 0) {
        raw[0] = 0;
    } else if (BN_bn2bin(bn, raw) != n) {
        free(raw);
        return (char*)0;
    }
    char* out = x07_ext_b64url_nopad(raw, (size_t)n, out_len);
    free(raw);
    return out;
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

int32_t x07_ext_openssl_ed25519_sign(
    const uint8_t* sk,
    uint32_t sk_len,
    const uint8_t* msg,
    uint32_t msg_len,
    uint8_t* out_sig,
    uint32_t out_sig_len
) {
    if (!sk || !out_sig) return 0;
    if (sk_len != 32u || out_sig_len != 64u) return 0;

    static const uint8_t zero = 0;
    if (!msg && msg_len == 0) msg = &zero;

    int nid = OBJ_txt2nid("ED25519");
    if (nid <= 0) return 0;

    EVP_PKEY* pkey = EVP_PKEY_new_raw_private_key(nid, NULL, sk, (size_t)sk_len);
    if (!pkey) return 0;

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_DigestSignInit(ctx, NULL, NULL, NULL, pkey) != 1) {
        EVP_MD_CTX_free(ctx);
        EVP_PKEY_free(pkey);
        return 0;
    }

    size_t siglen = (size_t)out_sig_len;
    int ok = EVP_DigestSign(ctx, out_sig, &siglen, msg, (size_t)msg_len) == 1 ? 1 : 0;
    EVP_MD_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    if (!ok) return 0;
    return siglen == 64u ? 1 : 0;
}

int32_t x07_ext_openssl_ed25519_sign_alloc(
    const uint8_t* sk,
    uint32_t sk_len,
    const uint8_t* msg,
    uint32_t msg_len
) {
    if (!sk) return 0;
    if (sk_len != 32u) return 0;

    uint8_t* sig = (uint8_t*)malloc(64);
    if (!sig) return 0;
    if (x07_ext_openssl_ed25519_sign(sk, sk_len, msg, msg_len, sig, 64u) != 1) {
        free(sig);
        return 0;
    }

    uint32_t slot = x07_ext_openssl_alloc_buf_slot();
    if (!slot) {
        free(sig);
        return 0;
    }
    g_bufs[slot] = sig;
    g_lens[slot] = 64u;
    return (int32_t)slot;
}

int32_t x07_ext_openssl_ed25519_pk_from_seed_alloc(
    const uint8_t* sk,
    uint32_t sk_len
) {
    if (!sk) return 0;
    if (sk_len != 32u) return 0;

    int nid = OBJ_txt2nid("ED25519");
    if (nid <= 0) return 0;

    EVP_PKEY* sk_pkey = EVP_PKEY_new_raw_private_key(nid, NULL, sk, (size_t)sk_len);
    if (!sk_pkey) return 0;

    uint8_t pk[32];
    size_t pk_len = sizeof(pk);
    if (EVP_PKEY_get_raw_public_key(sk_pkey, pk, &pk_len) != 1 || pk_len != 32u) {
        EVP_PKEY_free(sk_pkey);
        return 0;
    }
    EVP_PKEY_free(sk_pkey);

    uint8_t* out = (uint8_t*)malloc(32u);
    if (!out) return 0;
    memcpy(out, pk, 32u);

    uint32_t slot = x07_ext_openssl_alloc_buf_slot();
    if (!slot) {
        free(out);
        return 0;
    }
    g_bufs[slot] = out;
    g_lens[slot] = 32u;
    return (int32_t)slot;
}

int32_t x07_ext_openssl_ed25519_verify_sk(
    const uint8_t* sk,
    uint32_t sk_len,
    const uint8_t* msg,
    uint32_t msg_len,
    const uint8_t* sig,
    uint32_t sig_len
) {
    if (!sk || !sig) return 0;
    if (sk_len != 32u || sig_len != 64u) return 0;

    static const uint8_t zero = 0;
    if (!msg && msg_len == 0) msg = &zero;

    int nid = OBJ_txt2nid("ED25519");
    if (nid <= 0) return 0;

    EVP_PKEY* sk_pkey = EVP_PKEY_new_raw_private_key(nid, NULL, sk, (size_t)sk_len);
    if (!sk_pkey) return 0;

    uint8_t pk[32];
    size_t pk_len = sizeof(pk);
    if (EVP_PKEY_get_raw_public_key(sk_pkey, pk, &pk_len) != 1 || pk_len != 32u) {
        EVP_PKEY_free(sk_pkey);
        return 0;
    }

    EVP_PKEY* pk_pkey = EVP_PKEY_new_raw_public_key(nid, NULL, pk, pk_len);
    EVP_PKEY_free(sk_pkey);
    if (!pk_pkey) return 0;

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) {
        EVP_PKEY_free(pk_pkey);
        return 0;
    }

    if (EVP_DigestVerifyInit(ctx, NULL, NULL, NULL, pk_pkey) != 1) {
        EVP_MD_CTX_free(ctx);
        EVP_PKEY_free(pk_pkey);
        return 0;
    }

    int ok = EVP_DigestVerify(ctx, sig, (size_t)sig_len, msg, (size_t)msg_len) == 1 ? 1 : 0;
    EVP_MD_CTX_free(ctx);
    EVP_PKEY_free(pk_pkey);
    return ok;
}

int32_t x07_ext_openssl_rsa_pkcs1_sha256_verify(
    const uint8_t* n,
    uint32_t n_len,
    const uint8_t* e,
    uint32_t e_len,
    const uint8_t* msg,
    uint32_t msg_len,
    const uint8_t* sig,
    uint32_t sig_len
) {
    if (!n || !e || !sig) return 0;
    if (n_len == 0u || e_len == 0u || sig_len == 0u) return 0;
    if (sig_len != n_len) return 0;
    if (n_len > 2147483647u || e_len > 2147483647u || msg_len > 2147483647u) return 0;

    static const uint8_t zero = 0;
    if (!msg && msg_len == 0) msg = &zero;

    RSA* rsa = RSA_new();
    if (!rsa) return 0;

    BIGNUM* bn_n = BN_bin2bn(n, (int)n_len, NULL);
    BIGNUM* bn_e = BN_bin2bn(e, (int)e_len, NULL);
    if (!bn_n || !bn_e) {
        if (bn_n) BN_free(bn_n);
        if (bn_e) BN_free(bn_e);
        RSA_free(rsa);
        return 0;
    }

    if (RSA_set0_key(rsa, bn_n, bn_e, NULL) != 1) {
        BN_free(bn_n);
        BN_free(bn_e);
        RSA_free(rsa);
        return 0;
    }

    EVP_PKEY* pkey = EVP_PKEY_new();
    if (!pkey) {
        RSA_free(rsa);
        return 0;
    }
    if (EVP_PKEY_set1_RSA(pkey, rsa) != 1) {
        EVP_PKEY_free(pkey);
        RSA_free(rsa);
        return 0;
    }
    RSA_free(rsa);

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_DigestVerifyInit(ctx, NULL, EVP_sha256(), NULL, pkey) != 1) {
        EVP_MD_CTX_free(ctx);
        EVP_PKEY_free(pkey);
        return 0;
    }

    int ok = EVP_DigestVerify(ctx, sig, (size_t)sig_len, msg, (size_t)msg_len) == 1 ? 1 : 0;
    EVP_MD_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    return ok;
}

int32_t x07_ext_openssl_rsa_pkcs1_sha256_sign_alloc(
    const uint8_t* n,
    uint32_t n_len,
    const uint8_t* e,
    uint32_t e_len,
    const uint8_t* d,
    uint32_t d_len,
    const uint8_t* msg,
    uint32_t msg_len
) {
    if (!n || !e || !d) return 0;
    if (n_len == 0u || e_len == 0u || d_len == 0u) return 0;
    if (n_len > 2147483647u || e_len > 2147483647u || d_len > 2147483647u || msg_len > 2147483647u) return 0;

    static const uint8_t zero = 0;
    if (!msg && msg_len == 0) msg = &zero;

    RSA* rsa = RSA_new();
    if (!rsa) return 0;

    BIGNUM* bn_n = BN_bin2bn(n, (int)n_len, NULL);
    BIGNUM* bn_e = BN_bin2bn(e, (int)e_len, NULL);
    BIGNUM* bn_d = BN_bin2bn(d, (int)d_len, NULL);
    if (!bn_n || !bn_e || !bn_d) {
        if (bn_n) BN_free(bn_n);
        if (bn_e) BN_free(bn_e);
        if (bn_d) BN_free(bn_d);
        RSA_free(rsa);
        return 0;
    }

    if (RSA_set0_key(rsa, bn_n, bn_e, bn_d) != 1) {
        BN_free(bn_n);
        BN_free(bn_e);
        BN_free(bn_d);
        RSA_free(rsa);
        return 0;
    }

    EVP_PKEY* pkey = EVP_PKEY_new();
    if (!pkey) {
        RSA_free(rsa);
        return 0;
    }
    if (EVP_PKEY_set1_RSA(pkey, rsa) != 1) {
        EVP_PKEY_free(pkey);
        RSA_free(rsa);
        return 0;
    }
    RSA_free(rsa);

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_DigestSignInit(ctx, NULL, EVP_sha256(), NULL, pkey) != 1) {
        EVP_MD_CTX_free(ctx);
        EVP_PKEY_free(pkey);
        return 0;
    }

    uint8_t* sig = (uint8_t*)malloc(n_len);
    if (!sig) {
        EVP_MD_CTX_free(ctx);
        EVP_PKEY_free(pkey);
        return 0;
    }

    size_t siglen = (size_t)n_len;
    int ok = EVP_DigestSign(ctx, sig, &siglen, msg, (size_t)msg_len) == 1 ? 1 : 0;
    EVP_MD_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    if (!ok || siglen != (size_t)n_len) {
        free(sig);
        return 0;
    }

    uint32_t slot = x07_ext_openssl_alloc_buf_slot();
    if (!slot) {
        free(sig);
        return 0;
    }
    g_bufs[slot] = sig;
    g_lens[slot] = n_len;
    return (int32_t)slot;
}

int32_t x07_ext_openssl_rsa_rs256_private_jwk_generate_alloc(int32_t bits) {
    if (bits < 2048) bits = 2048;
    if (bits > 8192) return 0;

    RSA* rsa = RSA_new();
    if (!rsa) return 0;
    BIGNUM* e = BN_new();
    if (!e) {
        RSA_free(rsa);
        return 0;
    }
    if (BN_set_word(e, 65537ul) != 1) {
        BN_free(e);
        RSA_free(rsa);
        return 0;
    }
    if (RSA_generate_key_ex(rsa, bits, e, NULL) != 1) {
        BN_free(e);
        RSA_free(rsa);
        return 0;
    }
    BN_free(e);

    const BIGNUM* n_bn = (const BIGNUM*)0;
    const BIGNUM* e_bn = (const BIGNUM*)0;
    const BIGNUM* d_bn = (const BIGNUM*)0;
    const BIGNUM* p_bn = (const BIGNUM*)0;
    const BIGNUM* q_bn = (const BIGNUM*)0;
    const BIGNUM* dp_bn = (const BIGNUM*)0;
    const BIGNUM* dq_bn = (const BIGNUM*)0;
    const BIGNUM* qi_bn = (const BIGNUM*)0;
    RSA_get0_key(rsa, &n_bn, &e_bn, &d_bn);
    RSA_get0_factors(rsa, &p_bn, &q_bn);
    RSA_get0_crt_params(rsa, &dp_bn, &dq_bn, &qi_bn);
    if (!n_bn || !e_bn || !d_bn || !p_bn || !q_bn || !dp_bn || !dq_bn || !qi_bn) {
        RSA_free(rsa);
        return 0;
    }

    size_t n_len = 0u, e_len2 = 0u, d_len = 0u, p_len = 0u, q_len = 0u, dp_len = 0u, dq_len = 0u, qi_len = 0u;
    char* n = x07_ext_bn_b64url(n_bn, &n_len);
    char* e_s = x07_ext_bn_b64url(e_bn, &e_len2);
    char* d = x07_ext_bn_b64url(d_bn, &d_len);
    char* p = x07_ext_bn_b64url(p_bn, &p_len);
    char* q = x07_ext_bn_b64url(q_bn, &q_len);
    char* dp = x07_ext_bn_b64url(dp_bn, &dp_len);
    char* dq = x07_ext_bn_b64url(dq_bn, &dq_len);
    char* qi = x07_ext_bn_b64url(qi_bn, &qi_len);
    if (!n || !e_s || !d || !p || !q || !dp || !dq || !qi) {
        if (n) free(n);
        if (e_s) free(e_s);
        if (d) free(d);
        if (p) free(p);
        if (q) free(q);
        if (dp) free(dp);
        if (dq) free(dq);
        if (qi) free(qi);
        RSA_free(rsa);
        return 0;
    }

    const char* a0 = "{\"kty\":\"RSA\",\"n\":\"";
    const char* a1 = "\",\"e\":\"";
    const char* a2 = "\",\"d\":\"";
    const char* a3 = "\",\"p\":\"";
    const char* a4 = "\",\"q\":\"";
    const char* a5 = "\",\"dp\":\"";
    const char* a6 = "\",\"dq\":\"";
    const char* a7 = "\",\"qi\":\"";
    const char* a8 = "\"}\n";

    size_t total = strlen(a0) + n_len + strlen(a1) + e_len2 + strlen(a2) + d_len + strlen(a3) + p_len +
                   strlen(a4) + q_len + strlen(a5) + dp_len + strlen(a6) + dq_len + strlen(a7) + qi_len +
                   strlen(a8);
    uint8_t* out = (uint8_t*)malloc(total);
    if (!out) {
        free(n);
        free(e_s);
        free(d);
        free(p);
        free(q);
        free(dp);
        free(dq);
        free(qi);
        RSA_free(rsa);
        return 0;
    }

    uint8_t* w = out;
    memcpy(w, a0, strlen(a0));
    w += strlen(a0);
    memcpy(w, n, n_len);
    w += n_len;
    memcpy(w, a1, strlen(a1));
    w += strlen(a1);
    memcpy(w, e_s, e_len2);
    w += e_len2;
    memcpy(w, a2, strlen(a2));
    w += strlen(a2);
    memcpy(w, d, d_len);
    w += d_len;
    memcpy(w, a3, strlen(a3));
    w += strlen(a3);
    memcpy(w, p, p_len);
    w += p_len;
    memcpy(w, a4, strlen(a4));
    w += strlen(a4);
    memcpy(w, q, q_len);
    w += q_len;
    memcpy(w, a5, strlen(a5));
    w += strlen(a5);
    memcpy(w, dp, dp_len);
    w += dp_len;
    memcpy(w, a6, strlen(a6));
    w += strlen(a6);
    memcpy(w, dq, dq_len);
    w += dq_len;
    memcpy(w, a7, strlen(a7));
    w += strlen(a7);
    memcpy(w, qi, qi_len);
    w += qi_len;
    memcpy(w, a8, strlen(a8));
    w += strlen(a8);

    free(n);
    free(e_s);
    free(d);
    free(p);
    free(q);
    free(dp);
    free(dq);
    free(qi);
    RSA_free(rsa);

    uint32_t slot = x07_ext_openssl_alloc_buf_slot();
    if (!slot) {
        free(out);
        return 0;
    }
    g_bufs[slot] = out;
    g_lens[slot] = (uint32_t)total;
    return (int32_t)slot;
}

int32_t x07_ext_openssl_ecdsa_p256_sha256_verify_rawsig(
    const uint8_t* x,
    uint32_t x_len,
    const uint8_t* y,
    uint32_t y_len,
    const uint8_t* msg,
    uint32_t msg_len,
    const uint8_t* sig,
    uint32_t sig_len
) {
    if (!x || !y || !sig) return 0;
    if (x_len != 32u || y_len != 32u || sig_len != 64u) return 0;
    if (msg_len > 2147483647u) return 0;

    static const uint8_t zero = 0;
    if (!msg && msg_len == 0) msg = &zero;

    int nid = OBJ_txt2nid("prime256v1");
    if (nid <= 0) return 0;

    EC_KEY* ec = EC_KEY_new_by_curve_name(nid);
    if (!ec) return 0;

    BIGNUM* bn_x = BN_bin2bn(x, (int)x_len, NULL);
    BIGNUM* bn_y = BN_bin2bn(y, (int)y_len, NULL);
    if (!bn_x || !bn_y) {
        if (bn_x) BN_free(bn_x);
        if (bn_y) BN_free(bn_y);
        EC_KEY_free(ec);
        return 0;
    }

    if (EC_KEY_set_public_key_affine_coordinates(ec, bn_x, bn_y) != 1) {
        BN_free(bn_x);
        BN_free(bn_y);
        EC_KEY_free(ec);
        return 0;
    }

    BN_free(bn_x);
    BN_free(bn_y);

    if (EC_KEY_check_key(ec) != 1) {
        EC_KEY_free(ec);
        return 0;
    }

    EVP_PKEY* pkey = EVP_PKEY_new();
    if (!pkey) {
        EC_KEY_free(ec);
        return 0;
    }
    if (EVP_PKEY_set1_EC_KEY(pkey, ec) != 1) {
        EVP_PKEY_free(pkey);
        EC_KEY_free(ec);
        return 0;
    }
    EC_KEY_free(ec);

    ECDSA_SIG* ecdsa_sig = ECDSA_SIG_new();
    if (!ecdsa_sig) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    BIGNUM* r = BN_bin2bn(sig, 32, NULL);
    BIGNUM* s = BN_bin2bn(sig + 32, 32, NULL);
    if (!r || !s) {
        if (r) BN_free(r);
        if (s) BN_free(s);
        ECDSA_SIG_free(ecdsa_sig);
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (ECDSA_SIG_set0(ecdsa_sig, r, s) != 1) {
        BN_free(r);
        BN_free(s);
        ECDSA_SIG_free(ecdsa_sig);
        EVP_PKEY_free(pkey);
        return 0;
    }

    int der_len = i2d_ECDSA_SIG(ecdsa_sig, NULL);
    if (der_len <= 0) {
        ECDSA_SIG_free(ecdsa_sig);
        EVP_PKEY_free(pkey);
        return 0;
    }

    uint8_t* der = (uint8_t*)malloc((size_t)der_len);
    if (!der) {
        ECDSA_SIG_free(ecdsa_sig);
        EVP_PKEY_free(pkey);
        return 0;
    }
    uint8_t* p = der;
    if (i2d_ECDSA_SIG(ecdsa_sig, &p) != der_len) {
        free(der);
        ECDSA_SIG_free(ecdsa_sig);
        EVP_PKEY_free(pkey);
        return 0;
    }
    ECDSA_SIG_free(ecdsa_sig);

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) {
        free(der);
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_DigestVerifyInit(ctx, NULL, EVP_sha256(), NULL, pkey) != 1) {
        EVP_MD_CTX_free(ctx);
        free(der);
        EVP_PKEY_free(pkey);
        return 0;
    }

    int ok = EVP_DigestVerify(ctx, der, (size_t)der_len, msg, (size_t)msg_len) == 1 ? 1 : 0;
    EVP_MD_CTX_free(ctx);
    free(der);
    EVP_PKEY_free(pkey);
    return ok;
}
