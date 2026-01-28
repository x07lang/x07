#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#  include <arpa/inet.h>
#  include <netdb.h>

#include <curl/curl.h>

#define X07_EXT_CURL_MAX_HANDLES 4096u

static CURL* g_handles[X07_EXT_CURL_MAX_HANDLES];

static CURL* x07_ext_curl_get(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_CURL_MAX_HANDLES) return (CURL*)0;
    return g_handles[handle];
}

int32_t x07_ext_curl_global_init(uint32_t flags) {
    return (int32_t)curl_global_init((long)flags);
}

void x07_ext_curl_global_cleanup(void) {
    curl_global_cleanup();
}

uint32_t x07_ext_curl_easy_init(void) {
    CURL* h = curl_easy_init();
    if (!h) return 0;
    for (uint32_t i = 1; i < X07_EXT_CURL_MAX_HANDLES; i++) {
        if (!g_handles[i]) {
            g_handles[i] = h;
            return i;
        }
    }
    curl_easy_cleanup(h);
    return 0;
}

void x07_ext_curl_easy_cleanup(uint32_t handle) {
    CURL* h = x07_ext_curl_get(handle);
    if (!h) return;
    g_handles[handle] = (CURL*)0;
    curl_easy_cleanup(h);
}

int32_t x07_ext_curl_easy_setopt_long(uint32_t handle, uint32_t option, uint32_t value) {
    CURL* h = x07_ext_curl_get(handle);
    if (!h) return -1;
    return (int32_t)curl_easy_setopt(h, (CURLoption)(int32_t)option, (long)value);
}

int32_t x07_ext_curl_easy_setopt_ptr(uint32_t handle, uint32_t option, const void* value) {
    CURL* h = x07_ext_curl_get(handle);
    if (!h) return -1;
    return (int32_t)curl_easy_setopt(h, (CURLoption)(int32_t)option, value);
}

int32_t x07_ext_curl_easy_perform(uint32_t handle) {
    CURL* h = x07_ext_curl_get(handle);
    if (!h) return -1;
    return (int32_t)curl_easy_perform(h);
}

int32_t x07_ext_curl_easy_getinfo_long(uint32_t handle, uint32_t info, uint32_t* out) {
    CURL* h = x07_ext_curl_get(handle);
    if (!h) return -1;
    long v = 0;
    CURLcode rc = curl_easy_getinfo(h, (CURLINFO)(int32_t)info, &v);
    if (rc == 0 && out) *out = (uint32_t)v;
    return (int32_t)rc;
}

#define X07_EXT_CURL_MAX_BUFS 4096u

static uint8_t* g_bufs[X07_EXT_CURL_MAX_BUFS];
static uint32_t g_lens[X07_EXT_CURL_MAX_BUFS];

static uint32_t x07_ext_curl_alloc_buf_slot(void) {
    for (uint32_t i = 1; i < X07_EXT_CURL_MAX_BUFS; i++) {
        if (!g_bufs[i]) return i;
    }
    return 0;
}

uint32_t x07_ext_curl_buf_len(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_CURL_MAX_BUFS) return 0;
    return g_lens[handle];
}

const uint8_t* x07_ext_curl_buf_ptr(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_CURL_MAX_BUFS) return (const uint8_t*)0;
    return g_bufs[handle];
}

void x07_ext_curl_buf_free(uint32_t handle) {
    if (handle == 0 || handle >= X07_EXT_CURL_MAX_BUFS) return;
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

static int x07_ext_read_u32_le(const uint8_t* src, uint32_t src_len, uint32_t* pos, uint32_t* out) {
    uint32_t p = pos ? *pos : 0;
    if (!pos || !out) return 0;
    if (p > src_len || src_len - p < 4) return 0;
    uint32_t v = 0;
    v |= (uint32_t)src[p + 0];
    v |= (uint32_t)src[p + 1] << 8;
    v |= (uint32_t)src[p + 2] << 16;
    v |= (uint32_t)src[p + 3] << 24;
    *pos = p + 4;
    *out = v;
    return 1;
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

typedef struct x07ExtCurlBuf {
    uint8_t* data;
    uint32_t len;
    uint32_t cap;
    int too_large;
} x07ExtCurlBuf;

typedef struct x07ExtCurlHeaders {
    uint8_t* data;
    uint32_t len;
    uint32_t cap;
    uint32_t count;
    uint32_t max_headers;
    int too_large;
} x07ExtCurlHeaders;

typedef struct x07ExtCurlFileSink {
    FILE* f;
    uint32_t len;
    uint32_t cap;
    int too_large;
    int write_failed;
} x07ExtCurlFileSink;

static size_t x07_ext_curl_write_cb(char* ptr, size_t size, size_t nmemb, void* userdata) {
    x07ExtCurlBuf* b = (x07ExtCurlBuf*)userdata;
    if (!b || !ptr) return 0;
    size_t n = size * nmemb;
    if (n == 0) return 0;
    if (b->len > b->cap || n > (size_t)(b->cap - b->len)) {
        b->too_large = 1;
        return 0;
    }
    memcpy(b->data + b->len, (const uint8_t*)ptr, n);
    b->len += (uint32_t)n;
    return n;
}

static size_t x07_ext_curl_file_write_cb(char* ptr, size_t size, size_t nmemb, void* userdata) {
    x07ExtCurlFileSink* b = (x07ExtCurlFileSink*)userdata;
    if (!b || !b->f || !ptr) return 0;
    size_t n = size * nmemb;
    if (n == 0) return 0;
    if (b->len > b->cap || n > (size_t)(b->cap - b->len)) {
        b->too_large = 1;
        return 0;
    }
    size_t w = fwrite(ptr, 1, n, b->f);
    if (w != n) {
        b->write_failed = 1;
        return 0;
    }
    b->len += (uint32_t)n;
    return n;
}

static size_t x07_ext_curl_header_cb(char* ptr, size_t size, size_t nmemb, void* userdata) {
    x07ExtCurlHeaders* h = (x07ExtCurlHeaders*)userdata;
    if (!h || !ptr) return 0;

    size_t n = size * nmemb;
    if (n == 0) return 0;

    const uint8_t* b = (const uint8_t*)ptr;
    size_t m = n;
    while (m > 0 && (b[m - 1] == (uint8_t)'\n' || b[m - 1] == (uint8_t)'\r')) m--;

    if (m == 0) return n;
    if (m >= 5 && memcmp(b, "HTTP/", 5) == 0) return n;

    size_t colon = 0;
    while (colon < m && b[colon] != (uint8_t)':') colon++;
    if (colon == 0 || colon >= m) return n;

    size_t name_len = colon;
    size_t value_start = colon + 1;
    while (value_start < m && (b[value_start] == (uint8_t)' ' || b[value_start] == (uint8_t)'\t')) value_start++;
    size_t value_len = (value_start < m) ? (m - value_start) : 0;
    while (value_len > 0) {
        uint8_t c = b[value_start + value_len - 1];
        if (c != (uint8_t)' ' && c != (uint8_t)'\t') break;
        value_len--;
    }

    if (h->max_headers && h->count >= h->max_headers) {
        h->too_large = 1;
        return 0;
    }

    uint32_t need = 4u + (uint32_t)name_len + 4u + (uint32_t)value_len;
    if (h->len > h->cap || need > h->cap - h->len) {
        h->too_large = 1;
        return 0;
    }

    x07_ext_write_u32_le(h->data + h->len, (uint32_t)name_len);
    h->len += 4u;
    memcpy(h->data + h->len, b, name_len);
    h->len += (uint32_t)name_len;

    x07_ext_write_u32_le(h->data + h->len, (uint32_t)value_len);
    h->len += 4u;
    if (value_len) memcpy(h->data + h->len, b + value_start, value_len);
    h->len += (uint32_t)value_len;

    h->count += 1;
    return n;
}

static int x07_ext_env_is_1(const char* key) {
    const char* v = getenv(key);
    return v && strcmp(v, "1") == 0;
}

static int x07_ext_bytes_has_prefix(const uint8_t* b, uint32_t n, const char* prefix) {
    size_t m = strlen(prefix);
    if (n < (uint32_t)m) return 0;
    return memcmp(b, prefix, m) == 0;
}

static uint8_t x07_ext_ascii_lower(uint8_t c) {
    if (c >= (uint8_t)'A' && c <= (uint8_t)'Z') return (uint8_t)(c + 32);
    return c;
}

static int x07_ext_bytes_eq_nocase(const uint8_t* a, size_t a_len, const char* b, size_t b_len) {
    if (a_len != b_len) return 0;
    for (size_t i = 0; i < a_len; i++) {
        if (x07_ext_ascii_lower(a[i]) != x07_ext_ascii_lower((uint8_t)b[i])) return 0;
    }
    return 1;
}

static int x07_ext_split_next(const char** cursor, const char** out_start, size_t* out_len) {
    const char* p = cursor ? *cursor : NULL;
    if (!p || !out_start || !out_len) return 0;

    for (;;) {
        while (*p == ';' || *p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
        if (*p == 0) {
            *cursor = p;
            return 0;
        }

        const char* start = p;
        while (*p && *p != ';') p++;
        const char* end = p;
        while (end > start && (end[-1] == ' ' || end[-1] == '\t' || end[-1] == '\n' || end[-1] == '\r')) {
            end--;
        }

        *cursor = (*p == ';') ? p + 1 : p;

        if (end == start) {
            p = *cursor;
            continue;
        }

        *out_start = start;
        *out_len = (size_t)(end - start);
        return 1;
    }
}

static int x07_ext_parse_port(const char* s, size_t n, uint32_t* out_port) {
    if (!s || !out_port || n == 0) return 0;
    uint32_t v = 0;
    for (size_t i = 0; i < n; i++) {
        char c = s[i];
        if (c < '0' || c > '9') return 0;
        uint32_t d = (uint32_t)(c - '0');
        if (v > 65535u / 10u) return 0;
        v = v * 10u + d;
        if (v > 65535u) return 0;
    }
    *out_port = v;
    return 1;
}

static int x07_ext_parse_u32_dec(const char* s, size_t n, uint32_t* out) {
    if (!s || !out || n == 0) return 0;
    uint32_t x = 0;
    for (size_t i = 0; i < n; i++) {
        char c = s[i];
        if (c < '0' || c > '9') return 0;
        uint32_t d = (uint32_t)(c - '0');
        if (x > (UINT32_MAX / 10u)) return 0;
        x = x * 10u + d;
    }
    *out = x;
    return 1;
}

static int x07_ext_parse_ip_bytes(const uint8_t* s, size_t n, int* out_family, uint8_t out_addr[16]) {
    if (out_family) *out_family = 0;
    if (!s || n == 0 || !out_family || !out_addr) return 0;
    if (n > 255u) return 0;

    char buf[256];
    memcpy(buf, s, n);
    buf[n] = '\0';

    struct in_addr a4;
    memset(&a4, 0, sizeof(a4));
    if (inet_pton(AF_INET, buf, &a4) == 1) {
        *out_family = AF_INET;
        memset(out_addr, 0, 16);
        memcpy(out_addr, &a4, 4);
        return 1;
    }

    struct in6_addr a6;
    memset(&a6, 0, sizeof(a6));
    if (inet_pton(AF_INET6, buf, &a6) == 1) {
        *out_family = AF_INET6;
        memcpy(out_addr, &a6, 16);
        return 1;
    }

    return 0;
}

static uint32_t x07_ext_ipv4_u32(const uint8_t ip[4]) {
    return ((uint32_t)ip[0] << 24) | ((uint32_t)ip[1] << 16) | ((uint32_t)ip[2] << 8) | (uint32_t)ip[3];
}

static int x07_ext_ipv6_leq(const uint8_t a[16], const uint8_t b[16]) {
    return memcmp(a, b, 16) <= 0;
}

static int x07_ext_ipv6_geq(const uint8_t a[16], const uint8_t b[16]) {
    return memcmp(a, b, 16) >= 0;
}

static int x07_ext_ipv6_eq(const uint8_t a[16], const uint8_t b[16]) {
    return memcmp(a, b, 16) == 0;
}

static int x07_ext_ipv6_in_range(const uint8_t x[16], const uint8_t lo[16], const uint8_t hi[16]) {
    if (x07_ext_ipv6_leq(lo, hi)) {
        return x07_ext_ipv6_geq(x, lo) && x07_ext_ipv6_leq(x, hi);
    }
    return x07_ext_ipv6_geq(x, hi) && x07_ext_ipv6_leq(x, lo);
}

static int x07_ext_ipv4_in_range(uint32_t x, uint32_t lo, uint32_t hi) {
    if (lo <= hi) return x >= lo && x <= hi;
    return x >= hi && x <= lo;
}

static int x07_ext_ipv6_match_prefix(const uint8_t ip[16], const uint8_t base[16], uint32_t prefix_len) {
    if (prefix_len > 128u) return 0;
    uint32_t full = prefix_len / 8u;
    uint32_t rem = prefix_len % 8u;
    if (full) {
        if (memcmp(ip, base, full) != 0) return 0;
    }
    if (rem) {
        uint8_t mask = (uint8_t)(0xFFu << (8u - rem));
        if ((ip[full] & mask) != (base[full] & mask)) return 0;
    }
    return 1;
}

static int x07_ext_ipv4_match_prefix(uint32_t ip, uint32_t base, uint32_t prefix_len) {
    if (prefix_len > 32u) return 0;
    if (prefix_len == 0) return 1;
    uint32_t mask = (prefix_len == 32u) ? 0xFFFFFFFFu : (0xFFFFFFFFu << (32u - prefix_len));
    return (ip & mask) == (base & mask);
}

static int x07_ext_port_list_allows(const char* ports_s, size_t ports_n, uint32_t port) {
    size_t p = 0;
    while (p < ports_n) {
        while (p < ports_n && (ports_s[p] == ',' || ports_s[p] == ' ' || ports_s[p] == '\t' || ports_s[p] == '\n' || ports_s[p] == '\r')) {
            p++;
        }
        if (p >= ports_n) break;

        size_t q = p;
        while (q < ports_n && ports_s[q] != ',') q++;
        size_t q2 = q;
        while (q2 > p && (ports_s[q2 - 1] == ' ' || ports_s[q2 - 1] == '\t' || ports_s[q2 - 1] == '\n' || ports_s[q2 - 1] == '\r')) {
            q2--;
        }

        uint32_t pv = 0;
        if (x07_ext_parse_port(ports_s + p, q2 - p, &pv) && pv == port) return 1;
        p = (q < ports_n) ? (q + 1) : q;
    }
    return 0;
}

static int x07_ext_parse_http_host_port(
    const uint8_t* url,
    uint32_t url_len,
    uint32_t scheme_len,
    uint32_t default_port,
    const uint8_t** out_host,
    size_t* out_host_len,
    uint32_t* out_port
) {
    if (!url || !out_host || !out_host_len || !out_port) return 0;
    if (scheme_len >= url_len) return 0;

    uint32_t start = scheme_len;
    uint32_t end = start;
    while (end < url_len && url[end] != (uint8_t)'/' && url[end] != (uint8_t)'?' && url[end] != (uint8_t)'#') {
        end++;
    }
    if (end <= start) return 0;

    // Strip userinfo if present (take substring after last '@').
    uint32_t host_start = start;
    for (uint32_t i = start; i < end; i++) {
        if (url[i] == (uint8_t)'@') host_start = i + 1;
    }
    if (host_start >= end) return 0;

    uint32_t port = default_port;

    if (url[host_start] == (uint8_t)'[') {
        uint32_t close = host_start + 1;
        while (close < end && url[close] != (uint8_t)']') close++;
        if (close >= end) return 0;
        if (close <= host_start + 1) return 0;
        *out_host = url + host_start + 1;
        *out_host_len = (size_t)(close - (host_start + 1));

        if (close + 1 < end) {
            if (url[close + 1] != (uint8_t)':') return 0;
            const char* ps = (const char*)(url + close + 2);
            size_t pn = (size_t)(end - (close + 2));
            if (!x07_ext_parse_port(ps, pn, &port)) return 0;
        } else if (close + 1 != end) {
            return 0;
        }
    } else {
        uint32_t colon = UINT32_MAX;
        for (uint32_t i = host_start; i < end; i++) {
            if (url[i] == (uint8_t)':') colon = i;
        }

        if (colon != UINT32_MAX) {
            if (colon <= host_start) return 0;
            *out_host = url + host_start;
            *out_host_len = (size_t)(colon - host_start);
            const char* ps = (const char*)(url + colon + 1);
            size_t pn = (size_t)(end - (colon + 1));
            if (!x07_ext_parse_port(ps, pn, &port)) return 0;
        } else {
            *out_host = url + host_start;
            *out_host_len = (size_t)(end - host_start);
        }
    }

    if (*out_host_len == 0) return 0;
    *out_port = port;
    return 1;
}

static int x07_ext_host_port_allowed(const char* allow_hosts, const uint8_t* host, size_t host_len, uint32_t port) {
    if (!allow_hosts || !*allow_hosts || !host || host_len == 0) return 0;

    int host_family = 0;
    uint8_t host_ip[16];
    memset(host_ip, 0, sizeof(host_ip));
    int host_is_ip = x07_ext_parse_ip_bytes(host, host_len, &host_family, host_ip);

    const char* cur = allow_hosts;
    for (;;) {
        const char* entry = NULL;
        size_t entry_len = 0;
        if (!x07_ext_split_next(&cur, &entry, &entry_len)) return 0;

        size_t colon = (size_t)-1;
        for (size_t i = 0; i < entry_len; i++) {
            if (entry[i] == ':') colon = i;
        }
        if (colon == (size_t)-1) continue;

        const char* host_s = entry;
        size_t host_n = colon;
        const char* ports_s = entry + colon + 1;
        size_t ports_n = entry_len - colon - 1;

        if (host_n >= 2 && host_s[0] == '[' && host_s[host_n - 1] == ']') {
            const char* inner = (const char*)memchr(host_s + 1, ']', host_n - 2);
            if (!inner) {
                host_s += 1;
                host_n -= 2;
            }
        }

        if (!x07_ext_port_list_allows(ports_s, ports_n, port)) continue;

        if (host_is_ip) {
            const char* slash = (const char*)memchr(host_s, '/', host_n);
            const char* dash = (const char*)memchr(host_s, '-', host_n);

            if (slash) {
                size_t ip_n = (size_t)(slash - host_s);
                size_t pref_n = host_n - ip_n - 1;
                uint32_t pref = 0;
                if (!x07_ext_parse_u32_dec(slash + 1, pref_n, &pref)) continue;

                int base_family = 0;
                uint8_t base_ip[16];
                memset(base_ip, 0, sizeof(base_ip));
                const char* base_s = host_s;
                size_t base_n = ip_n;
                if (base_n >= 2 && base_s[0] == '[' && base_s[base_n - 1] == ']') {
                    base_s += 1;
                    base_n -= 2;
                }
                if (!x07_ext_parse_ip_bytes((const uint8_t*)base_s, base_n, &base_family, base_ip)) continue;

                if (host_family != base_family) continue;
                if (host_family == AF_INET) {
                    uint32_t hip = x07_ext_ipv4_u32(host_ip);
                    uint32_t bip = x07_ext_ipv4_u32(base_ip);
                    if (x07_ext_ipv4_match_prefix(hip, bip, pref)) return 1;
                } else if (host_family == AF_INET6) {
                    if (x07_ext_ipv6_match_prefix(host_ip, base_ip, pref)) return 1;
                }
                continue;
            }

            if (dash) {
                size_t lo_n = (size_t)(dash - host_s);
                size_t hi_n = host_n - lo_n - 1;
                if (hi_n == 0) continue;

                int lo_family = 0;
                uint8_t lo_ip[16];
                memset(lo_ip, 0, sizeof(lo_ip));
                int hi_family = 0;
                uint8_t hi_ip[16];
                memset(hi_ip, 0, sizeof(hi_ip));
                const char* lo_s = host_s;
                const char* hi_s = dash + 1;
                if (lo_n >= 2 && lo_s[0] == '[' && lo_s[lo_n - 1] == ']') {
                    lo_s += 1;
                    lo_n -= 2;
                }
                if (hi_n >= 2 && hi_s[0] == '[' && hi_s[hi_n - 1] == ']') {
                    hi_s += 1;
                    hi_n -= 2;
                }
                if (!x07_ext_parse_ip_bytes((const uint8_t*)lo_s, lo_n, &lo_family, lo_ip)) continue;
                if (!x07_ext_parse_ip_bytes((const uint8_t*)hi_s, hi_n, &hi_family, hi_ip)) continue;
                if (lo_family != hi_family || host_family != lo_family) continue;

                if (host_family == AF_INET) {
                    uint32_t hip = x07_ext_ipv4_u32(host_ip);
                    uint32_t lo = x07_ext_ipv4_u32(lo_ip);
                    uint32_t hi = x07_ext_ipv4_u32(hi_ip);
                    if (x07_ext_ipv4_in_range(hip, lo, hi)) return 1;
                } else if (host_family == AF_INET6) {
                    if (x07_ext_ipv6_in_range(host_ip, lo_ip, hi_ip)) return 1;
                }
                continue;
            }

            int pat_family = 0;
            uint8_t pat_ip[16];
            memset(pat_ip, 0, sizeof(pat_ip));
            if (x07_ext_parse_ip_bytes((const uint8_t*)host_s, host_n, &pat_family, pat_ip)) {
                if (host_family == pat_family) {
                    if (host_family == AF_INET) {
                        if (x07_ext_ipv4_u32(host_ip) == x07_ext_ipv4_u32(pat_ip)) return 1;
                    } else if (host_family == AF_INET6) {
                        if (x07_ext_ipv6_eq(host_ip, pat_ip)) return 1;
                    }
                }
                continue;
            }
        }

        if (x07_ext_bytes_eq_nocase(host, host_len, host_s, host_n)) return 1;
    }
}

static int x07_ext_path_has_hidden_segment(const uint8_t* path, uint32_t len) {
    if (!path || len == 0) return 0;
    uint32_t seg_start = 0;
    for (uint32_t i = 0; i <= len; i++) {
        uint8_t b = (i == len) ? (uint8_t)'/' : path[i];
        if (b == (uint8_t)'/') {
            if (i > seg_start && path[seg_start] == (uint8_t)'.') return 1;
            seg_start = i + 1;
        }
    }
    return 0;
}

static int x07_ext_path_has_root_prefix(const uint8_t* path, uint32_t path_len, const char* root, size_t root_len) {
    if (!path || !root || root_len == 0) return 0;
    while (root_len > 1 && (root[root_len - 1] == '/' || root[root_len - 1] == '\\')) root_len--;
    if (root_len == 1 && (root[0] == '/' || root[0] == '\\')) {
        return path_len > 0 && (path[0] == (uint8_t)'/' || path[0] == (uint8_t)'\\');
    }
    if (path_len < root_len) return 0;
    if (memcmp(path, root, root_len) != 0) return 0;
    if (path_len == root_len) return 1;
    uint8_t next = path[root_len];
    return next == (uint8_t)'/' || next == (uint8_t)'\\';
}

static int x07_ext_is_safe_rel_path(const uint8_t* path, uint32_t len) {
    if (!path || len == 0) return 0;
    if (path[0] == (uint8_t)'/') return 0;

    uint32_t seg_start = 0;
    for (uint32_t i = 0; i <= len; i++) {
        uint8_t b = (i == len) ? (uint8_t)'/' : path[i];
        if (i < len) {
            if (b == 0 || b == (uint8_t)'\\') return 0;
        }
        if (b == (uint8_t)'/') {
            uint32_t seg_len = i - seg_start;
            if (seg_len == 0) return 0;
            if (seg_len == 1 && path[seg_start] == (uint8_t)'.') return 0;
            if (seg_len == 2 && path[seg_start] == (uint8_t)'.' && path[seg_start + 1] == (uint8_t)'.') return 0;
            if (seg_len >= 5 && memcmp(path + seg_start, ".x07_", 5) == 0) return 0;
            seg_start = i + 1;
        }
    }
    return 1;
}

static char* x07_ext_bytes_to_cstr(const uint8_t* b, uint32_t len) {
    if (!b) return (char*)0;
    char* out = (char*)malloc((size_t)len + 1);
    if (!out) return (char*)0;
    if (len) memcpy(out, b, len);
    out[len] = '\0';
    return out;
}

static char* x07_ext_join_root_and_rel(const char* root, size_t root_len, const uint8_t* rel, uint32_t rel_len) {
    if (!root || !rel) return (char*)0;
    while (root_len > 0 && (root[root_len - 1] == '/' || root[root_len - 1] == '\\')) root_len--;
    if (root_len == 0) return (char*)0;
    size_t n = root_len + 1 + (size_t)rel_len;
    char* out = (char*)malloc(n + 1);
    if (!out) return (char*)0;
    memcpy(out, root, root_len);
    out[root_len] = '/';
    if (rel_len) memcpy(out + root_len + 1, rel, (size_t)rel_len);
    out[n] = '\0';
    return out;
}

static uint8_t* x07_ext_make_ok_http_resp(uint32_t status_code, const uint8_t* body, uint32_t body_len, uint32_t* out_len) {
    uint32_t payload_len = 1 + 4 + 4 + 4 + body_len;
    uint32_t doc_len = 1 + payload_len;
    uint8_t* buf = (uint8_t*)malloc(doc_len);
    if (!buf) return (uint8_t*)0;
    uint32_t p = 0;
    buf[p++] = 1;        // doc ok tag
    buf[p++] = 1;        // HttpRespV1 ver
    x07_ext_write_u32_le(buf + p, status_code);
    p += 4;
    x07_ext_write_u32_le(buf + p, 0); // header_count = 0 (v1)
    p += 4;
    x07_ext_write_u32_le(buf + p, body_len);
    p += 4;
    if (body_len && body) memcpy(buf + p, body, body_len);
    p += body_len;
    if (out_len) *out_len = p;
    return buf;
}

static uint8_t* x07_ext_make_ok_http_resp_v2(
    uint32_t status_code,
    const uint8_t* headers,
    uint32_t headers_len,
    uint32_t header_count,
    const uint8_t* body,
    uint32_t body_len,
    uint32_t* out_len
) {
    uint64_t payload_len = 1ull + 4ull + 4ull + (uint64_t)headers_len + 4ull + (uint64_t)body_len;
    uint64_t doc_len = 1ull + payload_len;
    if (doc_len > (uint64_t)UINT32_MAX) return (uint8_t*)0;

    uint8_t* buf = (uint8_t*)malloc((size_t)doc_len);
    if (!buf) return (uint8_t*)0;
    uint32_t p = 0;
    buf[p++] = 1;        // doc ok tag
    buf[p++] = 2;        // HttpRespV2 ver
    x07_ext_write_u32_le(buf + p, status_code);
    p += 4;
    x07_ext_write_u32_le(buf + p, header_count);
    p += 4;
    if (headers_len && headers) memcpy(buf + p, headers, headers_len);
    p += headers_len;
    x07_ext_write_u32_le(buf + p, body_len);
    p += 4;
    if (body_len && body) memcpy(buf + p, body, body_len);
    p += body_len;
    if (out_len) *out_len = p;
    return buf;
}

static uint8_t* x07_ext_make_ok_http_resp_v3_to_file(
    uint32_t status_code,
    const uint8_t* headers,
    uint32_t headers_len,
    uint32_t header_count,
    const uint8_t* path,
    uint32_t path_len,
    uint32_t body_len,
    uint32_t* out_len
) {
    uint64_t payload_len = 1ull + 4ull + 4ull + (uint64_t)headers_len + 4ull + (uint64_t)path_len + 4ull;
    uint64_t doc_len = 1ull + payload_len;
    if (doc_len > (uint64_t)UINT32_MAX) return (uint8_t*)0;

    uint8_t* buf = (uint8_t*)malloc((size_t)doc_len);
    if (!buf) return (uint8_t*)0;
    uint32_t p = 0;
    buf[p++] = 1;        // doc ok tag
    buf[p++] = 3;        // HttpRespToFileV1 ver
    x07_ext_write_u32_le(buf + p, status_code);
    p += 4;
    x07_ext_write_u32_le(buf + p, header_count);
    p += 4;
    if (headers_len && headers) memcpy(buf + p, headers, headers_len);
    p += headers_len;
    x07_ext_write_u32_le(buf + p, path_len);
    p += 4;
    if (path_len && path) memcpy(buf + p, path, path_len);
    p += path_len;
    x07_ext_write_u32_le(buf + p, body_len);
    p += 4;
    if (out_len) *out_len = p;
    return buf;
}

int32_t x07_ext_curl_http_request_alloc(
    const uint8_t* req,
    uint32_t req_len,
    uint32_t max_body_bytes,
    uint32_t* out_handle
) {
    if (out_handle) *out_handle = 0;

    uint32_t doc_len = 0;
    struct curl_slist* resolve_list = NULL;

    if (!req || req_len < 2) {
        uint8_t* doc = x07_ext_make_err_doc(2, &doc_len);
        if (!doc) return -1;
        uint32_t h = x07_ext_curl_alloc_buf_slot();
        if (!h) {
            free(doc);
            return -1;
        }
        g_bufs[h] = doc;
        g_lens[h] = doc_len;
        if (out_handle) *out_handle = h;
        return 0;
    }

    uint8_t ver = req[0];
    uint8_t method = req[1];
    uint32_t pos = 2;

    if (ver != 1 && ver != 2 && ver != 3) {
        uint8_t* doc = x07_ext_make_err_doc(2, &doc_len);
        if (!doc) return -1;
        uint32_t h = x07_ext_curl_alloc_buf_slot();
        if (!h) {
            free(doc);
            return -1;
        }
        g_bufs[h] = doc;
        g_lens[h] = doc_len;
        if (out_handle) *out_handle = h;
        return 0;
    }

    uint32_t url_len = 0;
    if (!x07_ext_read_u32_le(req, req_len, &pos, &url_len)) {
        uint8_t* doc = x07_ext_make_err_doc(2, &doc_len);
        if (!doc) return -1;
        uint32_t h = x07_ext_curl_alloc_buf_slot();
        if (!h) {
            free(doc);
            return -1;
        }
        g_bufs[h] = doc;
        g_lens[h] = doc_len;
        if (out_handle) *out_handle = h;
        return 0;
    }
    if (pos > req_len || url_len > req_len - pos) {
        uint8_t* doc = x07_ext_make_err_doc(2, &doc_len);
        if (!doc) return -1;
        uint32_t h = x07_ext_curl_alloc_buf_slot();
        if (!h) {
            free(doc);
            return -1;
        }
        g_bufs[h] = doc;
        g_lens[h] = doc_len;
        if (out_handle) *out_handle = h;
        return 0;
    }
    const uint8_t* url = req + pos;
    pos += url_len;

    if (memchr(url, 0, url_len) != NULL) {
        uint8_t* doc = x07_ext_make_err_doc(2, &doc_len);
        if (!doc) return -1;
        uint32_t h = x07_ext_curl_alloc_buf_slot();
        if (!h) {
            free(doc);
            return -1;
        }
        g_bufs[h] = doc;
        g_lens[h] = doc_len;
        if (out_handle) *out_handle = h;
        return 0;
    }

    uint32_t header_count = 0;
    if (!x07_ext_read_u32_le(req, req_len, &pos, &header_count)) {
        uint8_t* doc = x07_ext_make_err_doc(2, &doc_len);
        if (!doc) return -1;
        uint32_t h = x07_ext_curl_alloc_buf_slot();
        if (!h) {
            free(doc);
            return -1;
        }
        g_bufs[h] = doc;
        g_lens[h] = doc_len;
        if (out_handle) *out_handle = h;
        return 0;
    }

    struct curl_slist* headers = NULL;
    for (uint32_t i = 0; i < header_count; i++) {
        uint32_t name_len = 0;
        uint32_t val_len = 0;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &name_len)) goto invalid_req;
        if (pos > req_len || name_len > req_len - pos) goto invalid_req;
        const uint8_t* name = req + pos;
        pos += name_len;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &val_len)) goto invalid_req;
        if (pos > req_len || val_len > req_len - pos) goto invalid_req;
        const uint8_t* value = req + pos;
        pos += val_len;

        if (memchr(name, 0, name_len) != NULL) goto invalid_req;
        if (memchr(value, 0, val_len) != NULL) goto invalid_req;
        if (name_len == 0) goto invalid_req;

        size_t line_len = (size_t)name_len + 2 + (size_t)val_len;
        char* line = (char*)malloc(line_len + 1);
        if (!line) goto internal_err;
        memcpy(line, name, name_len);
        line[name_len] = ':';
        line[name_len + 1] = ' ';
        memcpy(line + name_len + 2, value, val_len);
        line[line_len] = '\0';

        struct curl_slist* new_list = curl_slist_append(headers, line);
        free(line);
        if (!new_list) goto internal_err;
        headers = new_list;
    }

    uint32_t body_len = 0;
    if (!x07_ext_read_u32_le(req, req_len, &pos, &body_len)) goto invalid_req;
    if (pos > req_len || body_len > req_len - pos) goto invalid_req;
    const uint8_t* body = req + pos;
    pos += body_len;

    if (method == 1) {
        if (body_len != 0) goto invalid_req;
    } else if (method != 2) {
        goto invalid_req;
    }

    uint32_t flags = 0;
    uint32_t timeout_s = 30;
    uint32_t max_redirects = 0;
    uint32_t max_header_bytes = 0;
    uint32_t max_headers = 0;
    uint32_t out_path_len = 0;
    const uint8_t* out_path = (const uint8_t*)0;

    if (ver == 1) {
        if (pos != req_len) goto invalid_req;
    } else if (ver == 2) {
        if (!x07_ext_read_u32_le(req, req_len, &pos, &flags)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &timeout_s)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &max_redirects)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &max_header_bytes)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &max_headers)) goto invalid_req;
        if (pos != req_len) goto invalid_req;

        if (timeout_s == 0) timeout_s = 30;
        if (timeout_s > 600u) goto invalid_req;
        if (max_redirects > 100u) goto invalid_req;
        if (max_header_bytes > 16u * 1024u * 1024u) goto invalid_req;
        if (max_headers > 65535u) goto invalid_req;
    } else {
        if (!x07_ext_read_u32_le(req, req_len, &pos, &flags)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &timeout_s)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &max_redirects)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &max_header_bytes)) goto invalid_req;
        if (!x07_ext_read_u32_le(req, req_len, &pos, &max_headers)) goto invalid_req;

        if (!x07_ext_read_u32_le(req, req_len, &pos, &out_path_len)) goto invalid_req;
        if (pos > req_len || out_path_len > req_len - pos) goto invalid_req;
        out_path = req + pos;
        pos += out_path_len;
        if (pos != req_len) goto invalid_req;

        if (timeout_s == 0) timeout_s = 30;
        if (timeout_s > 600u) goto invalid_req;
        if (max_redirects > 100u) goto invalid_req;
        if (max_header_bytes > 16u * 1024u * 1024u) goto invalid_req;
        if (max_headers > 65535u) goto invalid_req;
        if (out_path_len == 0) goto invalid_req;
        if (memchr(out_path, 0, out_path_len) != NULL) goto invalid_req;
        if (!x07_ext_is_safe_rel_path(out_path, out_path_len)) goto invalid_req;
    }

    int follow_location = (ver != 1 && (flags & 1u) != 0u) ? 1 : 0;

    int sandboxed = x07_ext_env_is_1("X07_OS_SANDBOXED");
    if (sandboxed) {
        if (follow_location) goto policy_denied;

        int is_http = x07_ext_bytes_has_prefix(url, url_len, "http://");
        int is_https = x07_ext_bytes_has_prefix(url, url_len, "https://");
        int is_file = x07_ext_bytes_has_prefix(url, url_len, "file://");

        if (is_http || is_https) {
            if (!x07_ext_env_is_1("X07_OS_NET")) goto policy_denied;
            if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_TCP")) goto policy_denied;
            if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_DNS")) goto policy_denied;
            const char* allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
            if (!allow_hosts || !*allow_hosts) goto policy_denied;

            const uint8_t* host = NULL;
            size_t host_len = 0;
            uint32_t port = 0;
            uint32_t scheme_len = is_https ? 8u : 7u;
            uint32_t def_port = is_https ? 443u : 80u;
            if (!x07_ext_parse_http_host_port(url, url_len, scheme_len, def_port, &host, &host_len, &port)) {
                goto invalid_req;
            }
            if (!x07_ext_host_port_allowed(allow_hosts, host, host_len, port)) goto policy_denied;

            int host_family = 0;
            uint8_t host_ip[16];
            memset(host_ip, 0, sizeof(host_ip));
            int host_is_ip = x07_ext_parse_ip_bytes(host, host_len, &host_family, host_ip);
            if (!host_is_ip) {
                if (host_len > 255u) goto invalid_req;
                char host_c[256];
                memcpy(host_c, host, host_len);
                host_c[host_len] = '\0';

                char service[16];
                snprintf(service, sizeof(service), "%u", (unsigned)port);

                struct addrinfo hints;
                memset(&hints, 0, sizeof(hints));
                hints.ai_family = AF_UNSPEC;
                hints.ai_socktype = SOCK_STREAM;
                hints.ai_protocol = IPPROTO_TCP;

                struct addrinfo* res = NULL;
                int gai = getaddrinfo(host_c, service, &hints, &res);
                if (gai != 0 || !res) goto policy_denied;

                int allowed = 0;
                for (struct addrinfo* it = res; it; it = it->ai_next) {
                    char ip_buf[INET6_ADDRSTRLEN];
                    memset(ip_buf, 0, sizeof(ip_buf));
                    if (it->ai_addr && it->ai_addr->sa_family == AF_INET) {
                        const struct sockaddr_in* in = (const struct sockaddr_in*)it->ai_addr;
                        if (!inet_ntop(AF_INET, &in->sin_addr, ip_buf, sizeof(ip_buf))) continue;
                    } else if (it->ai_addr && it->ai_addr->sa_family == AF_INET6) {
                        const struct sockaddr_in6* in6 = (const struct sockaddr_in6*)it->ai_addr;
                        if (!inet_ntop(AF_INET6, &in6->sin6_addr, ip_buf, sizeof(ip_buf))) continue;
                    } else {
                        continue;
                    }

                    if (!x07_ext_host_port_allowed(allow_hosts, (const uint8_t*)ip_buf, strlen(ip_buf), port)) continue;
                    allowed = 1;

                    char resolve_entry[512];
                    if (it->ai_addr->sa_family == AF_INET6) {
                        snprintf(
                            resolve_entry,
                            sizeof(resolve_entry),
                            "%.*s:%u:[%s]",
                            (int)host_len,
                            (const char*)host,
                            (unsigned)port,
                            ip_buf
                        );
                    } else {
                        snprintf(
                            resolve_entry,
                            sizeof(resolve_entry),
                            "%.*s:%u:%s",
                            (int)host_len,
                            (const char*)host,
                            (unsigned)port,
                            ip_buf
                        );
                    }
                    struct curl_slist* next = curl_slist_append(resolve_list, resolve_entry);
                    if (!next) {
                        freeaddrinfo(res);
                        goto internal_err;
                    }
                    resolve_list = next;
                }
                freeaddrinfo(res);

                if (!allowed) goto policy_denied;
            }
        } else if (is_file) {
            if (!x07_ext_env_is_1("X07_OS_FS")) goto policy_denied;
            if (!x07_ext_bytes_has_prefix(url, url_len, "file:///")) goto invalid_req;

            const uint8_t* path = url + 7;
            uint32_t path_len = url_len - 7;
            if (x07_ext_env_is_1("X07_OS_DENY_HIDDEN") && x07_ext_path_has_hidden_segment(path, path_len)) {
                goto policy_denied;
            }

            const char* roots = getenv("X07_OS_FS_READ_ROOTS");
            if (!roots || !*roots) goto policy_denied;
            const char* cur = roots;
            const char* root = NULL;
            size_t root_len = 0;
            int ok = 0;
            while (x07_ext_split_next(&cur, &root, &root_len)) {
                if (x07_ext_path_has_root_prefix(path, path_len, root, root_len)) {
                    ok = 1;
                    break;
                }
            }
            if (!ok) goto policy_denied;
        } else {
            goto invalid_req;
        }

        if (ver == 3) {
            if (!x07_ext_env_is_1("X07_OS_FS")) goto policy_denied;
            if (x07_ext_env_is_1("X07_OS_DENY_HIDDEN") && x07_ext_path_has_hidden_segment(out_path, out_path_len)) {
                goto policy_denied;
            }
            const char* roots = getenv("X07_OS_FS_WRITE_ROOTS");
            if (!roots || !*roots) goto policy_denied;
        }
    }

    char* url_c = (char*)malloc((size_t)url_len + 1);
    if (!url_c) goto internal_err;
    memcpy(url_c, url, url_len);
    url_c[url_len] = '\0';

    CURL* h = curl_easy_init();
    if (!h) {
        free(url_c);
        goto internal_err;
    }
    if (resolve_list) {
        (void)curl_easy_setopt(h, CURLOPT_RESOLVE, resolve_list);
    }

    x07ExtCurlBuf buf;
    buf.data = (uint8_t*)0;
    buf.len = 0;
    buf.cap = max_body_bytes;
    buf.too_large = 0;

    char* out_path_opened = (char*)0;
    uint32_t out_path_opened_len = 0;

    x07ExtCurlFileSink file_sink;
    file_sink.f = (FILE*)0;
    file_sink.len = 0;
    file_sink.cap = max_body_bytes;
    file_sink.too_large = 0;
    file_sink.write_failed = 0;

    if (ver != 3) {
        buf.data = (uint8_t*)malloc((size_t)max_body_bytes);
        buf.len = 0;
        buf.cap = max_body_bytes;
        buf.too_large = 0;
        if (!buf.data && max_body_bytes != 0) {
            curl_easy_cleanup(h);
            free(url_c);
            goto internal_err;
        }
    } else {
        if (sandboxed) {
            const char* roots = getenv("X07_OS_FS_WRITE_ROOTS");
            if (!roots || !*roots) {
                curl_easy_cleanup(h);
                free(url_c);
                goto policy_denied;
            }
            const char* cur = roots;
            const char* root = NULL;
            size_t root_len = 0;
            while (x07_ext_split_next(&cur, &root, &root_len)) {
                char* p = x07_ext_join_root_and_rel(root, root_len, out_path, out_path_len);
                if (!p) continue;
                FILE* f = fopen(p, "wb");
                if (f) {
                    out_path_opened = p;
                    file_sink.f = f;
                    break;
                }
                free(p);
            }
            if (!file_sink.f) {
                curl_easy_cleanup(h);
                free(url_c);
                goto policy_denied;
            }
        } else {
            out_path_opened = x07_ext_bytes_to_cstr(out_path, out_path_len);
            if (!out_path_opened) {
                curl_easy_cleanup(h);
                free(url_c);
                goto internal_err;
            }
            file_sink.f = fopen(out_path_opened, "wb");
            if (!file_sink.f) {
                free(out_path_opened);
                curl_easy_cleanup(h);
                free(url_c);
                goto internal_err;
            }
        }

        size_t n_opened = strlen(out_path_opened);
        if (n_opened > (size_t)UINT32_MAX) {
            fclose(file_sink.f);
            file_sink.f = (FILE*)0;
            (void)remove(out_path_opened);
            free(out_path_opened);
            curl_easy_cleanup(h);
            free(url_c);
            goto internal_err;
        }
        out_path_opened_len = (uint32_t)n_opened;
    }

    (void)curl_easy_setopt(h, CURLOPT_URL, url_c);
    (void)curl_easy_setopt(h, CURLOPT_FOLLOWLOCATION, follow_location ? 1L : 0L);
    if (follow_location) {
        long max_redirs = (max_redirects == 0) ? 5L : (long)max_redirects;
        (void)curl_easy_setopt(h, CURLOPT_MAXREDIRS, max_redirs);
    }
    (void)curl_easy_setopt(h, CURLOPT_TIMEOUT, (long)timeout_s);
    if (ver == 3) {
        (void)curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, x07_ext_curl_file_write_cb);
        (void)curl_easy_setopt(h, CURLOPT_WRITEDATA, &file_sink);
    } else {
        (void)curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, x07_ext_curl_write_cb);
        (void)curl_easy_setopt(h, CURLOPT_WRITEDATA, &buf);
    }
    if (headers) (void)curl_easy_setopt(h, CURLOPT_HTTPHEADER, headers);

    x07ExtCurlHeaders resp_headers;
    int capture_headers = (ver != 1 && max_header_bytes != 0 && max_headers != 0);
    resp_headers.data = (uint8_t*)0;
    resp_headers.len = 0;
    resp_headers.cap = 0;
    resp_headers.count = 0;
    resp_headers.max_headers = 0;
    resp_headers.too_large = 0;
    if (capture_headers) {
        resp_headers.data = (uint8_t*)malloc((size_t)max_header_bytes);
        resp_headers.len = 0;
        resp_headers.cap = max_header_bytes;
        resp_headers.count = 0;
        resp_headers.max_headers = max_headers;
        resp_headers.too_large = 0;
        if (!resp_headers.data) {
            curl_easy_cleanup(h);
            free(url_c);
            if (buf.data) free(buf.data);
            if (file_sink.f) {
                fclose(file_sink.f);
                file_sink.f = (FILE*)0;
                if (out_path_opened) (void)remove(out_path_opened);
            }
            if (out_path_opened) free(out_path_opened);
            goto internal_err;
        }
        (void)curl_easy_setopt(h, CURLOPT_HEADERFUNCTION, x07_ext_curl_header_cb);
        (void)curl_easy_setopt(h, CURLOPT_HEADERDATA, &resp_headers);
    }

    if (method == 1) {
        (void)curl_easy_setopt(h, CURLOPT_HTTPGET, 1L);
    } else {
        (void)curl_easy_setopt(h, CURLOPT_POST, 1L);
        (void)curl_easy_setopt(h, CURLOPT_POSTFIELDS, body_len ? (const void*)body : "");
        (void)curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)body_len);
    }

    CURLcode rc = curl_easy_perform(h);
    long status = 0;
    (void)curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);

    uint32_t err_code = 0;
    if (capture_headers && resp_headers.too_large) {
        err_code = 4;
    } else if (ver == 3 && file_sink.too_large) {
        err_code = 4;
    } else if (ver == 3 && file_sink.write_failed) {
        err_code = 8;
    } else if (buf.too_large) {
        err_code = 4;
    } else if (rc != CURLE_OK) {
        if (rc == CURLE_OPERATION_TIMEDOUT) {
            err_code = 3;
        } else if (rc == CURLE_COULDNT_RESOLVE_HOST) {
            err_code = 5;
        } else if (rc == CURLE_COULDNT_CONNECT) {
            err_code = 6;
        } else if (
            rc == CURLE_SSL_CONNECT_ERROR ||
            rc == CURLE_PEER_FAILED_VERIFICATION ||
            rc == CURLE_SSL_CACERT ||
            rc == CURLE_SSL_CACERT_BADFILE ||
            rc == CURLE_USE_SSL_FAILED
        ) {
            err_code = 7;
        } else if (rc == CURLE_WRITE_ERROR) {
            err_code = 4;
        } else {
            err_code = 8;
        }
    }

    uint8_t* doc = NULL;
    if (err_code != 0) {
        doc = x07_ext_make_err_doc(err_code, &doc_len);
    } else {
        if (ver == 1) {
            doc = x07_ext_make_ok_http_resp((uint32_t)status, buf.data, buf.len, &doc_len);
        } else if (ver == 2) {
            doc = x07_ext_make_ok_http_resp_v2(
                (uint32_t)status,
                capture_headers ? resp_headers.data : (const uint8_t*)0,
                capture_headers ? resp_headers.len : 0u,
                capture_headers ? resp_headers.count : 0u,
                buf.data,
                buf.len,
                &doc_len
            );
        } else {
            doc = x07_ext_make_ok_http_resp_v3_to_file(
                (uint32_t)status,
                capture_headers ? resp_headers.data : (const uint8_t*)0,
                capture_headers ? resp_headers.len : 0u,
                capture_headers ? resp_headers.count : 0u,
                (const uint8_t*)out_path_opened,
                out_path_opened_len,
                file_sink.len,
                &doc_len
            );
        }
    }

    if (headers) curl_slist_free_all(headers);
    if (resolve_list) curl_slist_free_all(resolve_list);
    curl_easy_cleanup(h);
    free(url_c);
    if (buf.data) free(buf.data);
    if (capture_headers && resp_headers.data) free(resp_headers.data);
    if (file_sink.f) {
        fclose(file_sink.f);
        file_sink.f = (FILE*)0;
    }
    if ((err_code != 0 || !doc) && out_path_opened) {
        (void)remove(out_path_opened);
    }
    if (out_path_opened) free(out_path_opened);

    if (!doc) return -1;
    uint32_t slot = x07_ext_curl_alloc_buf_slot();
    if (!slot) {
        free(doc);
        return -1;
    }
    g_bufs[slot] = doc;
    g_lens[slot] = doc_len;
    if (out_handle) *out_handle = slot;
    return 0;

policy_denied:
    if (headers) curl_slist_free_all(headers);
    if (resolve_list) curl_slist_free_all(resolve_list);
    {
        uint8_t* doc2 = x07_ext_make_err_doc(1, &doc_len);
        if (!doc2) return -1;
        uint32_t slot2 = x07_ext_curl_alloc_buf_slot();
        if (!slot2) {
            free(doc2);
            return -1;
        }
        g_bufs[slot2] = doc2;
        g_lens[slot2] = doc_len;
        if (out_handle) *out_handle = slot2;
        return 0;
    }

invalid_req:
    if (headers) curl_slist_free_all(headers);
    if (resolve_list) curl_slist_free_all(resolve_list);
    {
        uint8_t* doc2 = x07_ext_make_err_doc(2, &doc_len);
        if (!doc2) return -1;
        uint32_t slot2 = x07_ext_curl_alloc_buf_slot();
        if (!slot2) {
            free(doc2);
            return -1;
        }
        g_bufs[slot2] = doc2;
        g_lens[slot2] = doc_len;
        if (out_handle) *out_handle = slot2;
        return 0;
    }

internal_err:
    if (headers) curl_slist_free_all(headers);
    if (resolve_list) curl_slist_free_all(resolve_list);
    {
        uint8_t* doc2 = x07_ext_make_err_doc(8, &doc_len);
        if (!doc2) return -1;
        uint32_t slot2 = x07_ext_curl_alloc_buf_slot();
        if (!slot2) {
            free(doc2);
            return -1;
        }
        g_bufs[slot2] = doc2;
        g_lens[slot2] = doc_len;
        if (out_handle) *out_handle = slot2;
        return 0;
    }
}
