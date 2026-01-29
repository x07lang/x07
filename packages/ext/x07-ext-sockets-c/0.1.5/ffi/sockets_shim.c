#include <limits.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#  include <arpa/inet.h>
#  include <errno.h>
#  include <fcntl.h>
#  include <netdb.h>
#  include <netinet/in.h>
#  include <poll.h>
#  include <sys/socket.h>
#  include <sys/types.h>
#  include <unistd.h>
typedef int x07_sock_t;
#  define X07_SOCK_INVALID (-1)
#  define x07_sock_close close
#  define x07_sock_errno() (errno)
typedef socklen_t x07_socklen_t;

#include <openssl/err.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>

// Net error codes (pinned in docs/net/errors-v1.md).
enum {
  X07_NET_ERR_POLICY_DENIED = 1,
  X07_NET_ERR_INVALID_REQ = 2,
  X07_NET_ERR_TIMEOUT = 3,
  X07_NET_ERR_TOO_LARGE = 4,
  X07_NET_ERR_DNS = 5,
  X07_NET_ERR_CONNECT = 6,
  X07_NET_ERR_TLS = 7,
  X07_NET_ERR_INTERNAL = 8,
};

// V1 NetAddr tags (pinned in docs/net/net-v1.md).
enum {
  X07_NET_ADDR_TAG_IPV4 = 1,
  X07_NET_ADDR_TAG_IPV6 = 2,
  X07_NET_ADDR_TAG_DNS = 3,
};

enum {
  X07_SOCK_KIND_NONE = 0,
  X07_SOCK_KIND_TCP_STREAM = 1,
  X07_SOCK_KIND_TCP_LISTENER = 2,
  X07_SOCK_KIND_UDP = 3,
  X07_SOCK_KIND_TLS_STREAM = 4,
};

typedef struct x07SockEntry {
  uint8_t alive;
  uint8_t kind;
  uint8_t closed;
  uint8_t _pad;
  x07_sock_t fd;
} x07SockEntry;

#define X07_EXT_SOCKETS_MAX_SOCKS 4096u
static x07SockEntry g_socks[X07_EXT_SOCKETS_MAX_SOCKS + 1];
static uint8_t g_sock_caps_ready[X07_EXT_SOCKETS_MAX_SOCKS + 1];

typedef struct x07TlsEntry {
  SSL_CTX* ctx;
  SSL* ssl;
} x07TlsEntry;

static x07TlsEntry g_tls[X07_EXT_SOCKETS_MAX_SOCKS + 1];

static uint32_t x07_ext_sockets_alloc_sock_slot(uint8_t kind, x07_sock_t fd) {
  for (uint32_t i = 1; i <= X07_EXT_SOCKETS_MAX_SOCKS; i++) {
    if (!g_socks[i].alive) {
      g_socks[i].alive = 1;
      g_socks[i].kind = kind;
      g_socks[i].closed = 0;
      g_socks[i].fd = fd;
      memset(&g_tls[i], 0, sizeof(g_tls[i]));
      g_sock_caps_ready[i] = 0;
      return i;
    }
  }
  return 0;
}

static x07SockEntry* x07_ext_sockets_sock_ptr(uint32_t h) {
  if (h == 0 || h > X07_EXT_SOCKETS_MAX_SOCKS) return NULL;
  x07SockEntry* e = &g_socks[h];
  if (!e->alive) return NULL;
  return e;
}

static void x07_ext_sockets_tls_drop_in_place(uint32_t h) {
  if (h == 0 || h > X07_EXT_SOCKETS_MAX_SOCKS) return;
  SSL* ssl = g_tls[h].ssl;
  SSL_CTX* ctx = g_tls[h].ctx;
  g_tls[h].ssl = NULL;
  g_tls[h].ctx = NULL;
  if (ssl) {
    (void)SSL_shutdown(ssl);
    SSL_free(ssl);
  }
  if (ctx) {
    SSL_CTX_free(ctx);
  }
}

static void x07_ext_sockets_sock_close_in_place(x07SockEntry* e) {
  if (!e || !e->alive || e->closed) return;
  uint32_t h = (uint32_t)(e - g_socks);
  if (e->kind == X07_SOCK_KIND_TLS_STREAM) {
    x07_ext_sockets_tls_drop_in_place(h);
  }
  if (e->fd != X07_SOCK_INVALID) {
    (void)x07_sock_close(e->fd);
  }
  e->fd = X07_SOCK_INVALID;
  e->closed = 1;
}

static int32_t x07_ext_sockets_sock_close(uint32_t h, uint8_t expected_kind) {
  x07SockEntry* e = x07_ext_sockets_sock_ptr(h);
  if (!e || e->kind != expected_kind) return 0;
  if (e->closed) return 0;
  x07_ext_sockets_sock_close_in_place(e);
  return 1;
}

static int32_t x07_ext_sockets_sock_drop(uint32_t h, uint8_t expected_kind) {
  x07SockEntry* e = x07_ext_sockets_sock_ptr(h);
  if (!e || e->kind != expected_kind) return 0;
  x07_ext_sockets_sock_close_in_place(e);
  g_sock_caps_ready[h] = 0;
  memset(e, 0, sizeof(*e));
  return 1;
}

// ---- Output buffer slots (like ext-curl-c, but with sockets-specific names) ----

#define X07_EXT_SOCKETS_MAX_BUFS 4096u
static uint8_t* g_bufs[X07_EXT_SOCKETS_MAX_BUFS + 1];
static uint32_t g_lens[X07_EXT_SOCKETS_MAX_BUFS + 1];

static uint32_t x07_ext_sockets_alloc_buf_slot(void) {
  for (uint32_t i = 1; i <= X07_EXT_SOCKETS_MAX_BUFS; i++) {
    if (!g_bufs[i]) return i;
  }
  return 0;
}

int32_t x07_ext_sockets_buf_len(int32_t handle) {
  if (handle <= 0 || (uint32_t)handle > X07_EXT_SOCKETS_MAX_BUFS) return -1;
  uint32_t h = (uint32_t)handle;
  if (!g_bufs[h]) return -1;
  if (g_lens[h] > (uint32_t)INT32_MAX) return -1;
  return (int32_t)g_lens[h];
}

const uint8_t* x07_ext_sockets_buf_ptr(int32_t handle) {
  if (handle <= 0 || (uint32_t)handle > X07_EXT_SOCKETS_MAX_BUFS) return NULL;
  return g_bufs[(uint32_t)handle];
}

void x07_ext_sockets_buf_free(int32_t handle) {
  if (handle <= 0 || (uint32_t)handle > X07_EXT_SOCKETS_MAX_BUFS) return;
  uint32_t h = (uint32_t)handle;
  if (g_bufs[h]) free(g_bufs[h]);
  g_bufs[h] = NULL;
  g_lens[h] = 0;
}

// ---- Bytes helpers ----

static void x07_ext_write_u32_le(uint8_t* out, uint32_t x) {
  out[0] = (uint8_t)(x & 255u);
  out[1] = (uint8_t)((x >> 8) & 255u);
  out[2] = (uint8_t)((x >> 16) & 255u);
  out[3] = (uint8_t)((x >> 24) & 255u);
}

static int x07_ext_read_u32_le(const uint8_t* b, uint32_t n, uint32_t* pos, uint32_t* out) {
  if (!b || !pos || !out) return 0;
  uint32_t p = *pos;
  if (p > n || n - p < 4u) return 0;
  uint32_t x = 0;
  x |= (uint32_t)b[p + 0];
  x |= (uint32_t)b[p + 1] << 8;
  x |= (uint32_t)b[p + 2] << 16;
  x |= (uint32_t)b[p + 3] << 24;
  *pos = p + 4u;
  *out = x;
  return 1;
}

static uint8_t* x07_ext_make_err_doc(uint32_t code, uint32_t* out_len) {
  uint8_t* buf = (uint8_t*)malloc(9);
  if (!buf) return NULL;
  buf[0] = 0;
  x07_ext_write_u32_le(buf + 1, code);
  x07_ext_write_u32_le(buf + 5, 0);
  if (out_len) *out_len = 9;
  return buf;
}

static int x07_ext_env_is_1(const char* key) {
  const char* v = getenv(key);
  return v && strcmp(v, "1") == 0;
}

static int x07_ext_split_next(const char** s, const char** out_p, size_t* out_n) {
  if (!s || !*s || !out_p || !out_n) return 0;
  const char* cur = *s;
  if (!cur || !*cur) return 0;
  const char* start = cur;
  while (*cur && *cur != ';') cur++;
  const char* end = cur;
  if (*cur == ';') cur++;
  *s = cur;
  *out_p = start;
  *out_n = (size_t)(end - start);
  return 1;
}

static uint8_t x07_ext_ascii_lower(uint8_t c) {
  if (c >= (uint8_t)'A' && c <= (uint8_t)'Z') return (uint8_t)(c + 32);
  return c;
}

static int x07_ext_bytes_eq_nocase(const uint8_t* a, size_t a_len, const char* b, size_t b_len) {
  if (!a || !b) return 0;
  if (a_len != b_len) return 0;
  for (size_t i = 0; i < a_len; i++) {
    if (x07_ext_ascii_lower(a[i]) != x07_ext_ascii_lower((uint8_t)b[i])) return 0;
  }
  return 1;
}

static int x07_ext_parse_port(const char* s, size_t n, uint32_t* out) {
  if (!s || !out || n == 0) return 0;
  uint32_t x = 0;
  for (size_t i = 0; i < n; i++) {
    char c = s[i];
    if (c < '0' || c > '9') return 0;
    uint32_t d = (uint32_t)(c - '0');
    if (x > (UINT32_MAX / 10u)) return 0;
    x = x * 10u + d;
    if (x > 65535u) return 0;
  }
  *out = x;
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

  // Policy host strings are max 255; hard-cap any dynamic inputs here too.
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

    // First: ports must match.
    if (!x07_ext_port_list_allows(ports_s, ports_n, port)) continue;

    // Then: host match (exact DNS name, exact IP, CIDR, or range).
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

// ---- NetAddr/NetCaps parsing ----

typedef struct x07NetAddrV1 {
  uint32_t tag;
  uint32_t port;
  const uint8_t* ip;
  uint32_t ip_len;
  const uint8_t* dns;
  uint32_t dns_len;
} x07NetAddrV1;

typedef struct x07NetCapsV1 {
  uint32_t connect_timeout_ms;
  uint32_t io_timeout_ms;
  uint32_t max_read_bytes;
  uint32_t max_write_bytes;
} x07NetCapsV1;

typedef struct x07TlsClientCfgV1 {
  uint32_t verify_peer;
  const uint8_t* sni;
  uint32_t sni_len;
} x07TlsClientCfgV1;

static int x07_ext_parse_caps_v1(const uint8_t* caps, uint32_t caps_len, x07NetCapsV1* out) {
  if (!out) return 0;

  // Defaults (v1).
  out->connect_timeout_ms = 30u * 1000u;
  out->io_timeout_ms = 30u * 1000u;
  out->max_read_bytes = 1024u * 1024u;
  out->max_write_bytes = 1024u * 1024u;

  if (!caps || caps_len < 24u) return 1;

  uint32_t pos = 0;
  uint32_t ver = 0;
  if (!x07_ext_read_u32_le(caps, caps_len, &pos, &ver)) return 0;
  if (ver != 1u) return 0;

  uint32_t connect_timeout_ms = 0;
  uint32_t io_timeout_ms = 0;
  uint32_t max_read_bytes = 0;
  uint32_t max_write_bytes = 0;
  uint32_t reserved = 0;

  if (!x07_ext_read_u32_le(caps, caps_len, &pos, &connect_timeout_ms)) return 0;
  if (!x07_ext_read_u32_le(caps, caps_len, &pos, &io_timeout_ms)) return 0;
  if (!x07_ext_read_u32_le(caps, caps_len, &pos, &max_read_bytes)) return 0;
  if (!x07_ext_read_u32_le(caps, caps_len, &pos, &max_write_bytes)) return 0;
  if (!x07_ext_read_u32_le(caps, caps_len, &pos, &reserved)) return 0;
  if (pos != caps_len) return 0;

  if (connect_timeout_ms != 0) out->connect_timeout_ms = connect_timeout_ms;
  if (io_timeout_ms != 0) out->io_timeout_ms = io_timeout_ms;
  if (max_read_bytes != 0) out->max_read_bytes = max_read_bytes;
  if (max_write_bytes != 0) out->max_write_bytes = max_write_bytes;

  // Hard upper bounds to keep behavior resource-bounded.
  if (out->connect_timeout_ms > 10u * 60u * 1000u) return 0;
  if (out->io_timeout_ms > 10u * 60u * 1000u) return 0;
  if (out->max_read_bytes > 64u * 1024u * 1024u) return 0;
  if (out->max_write_bytes > 64u * 1024u * 1024u) return 0;

  return 1;
}

// ---- iface streaming support (std.io / bufread) ----

static uint32_t x07_ext_netaddr_ipv4_len(void);
static uint32_t x07_ext_netaddr_ipv6_len(void);
static void x07_ext_build_netaddr_ipv4(uint8_t* out, const uint8_t ip[4], uint32_t port);
static void x07_ext_build_netaddr_ipv6(uint8_t* out, const uint8_t ip[16], uint32_t port);
static int x07_ext_wsa_ready(void);
static int x07_ext_poll_one(x07_sock_t fd, int events, int timeout_ms, int* out_revents);

typedef uint32_t (*x07_rt_ext_io_reader_read_fn_t)(uint32_t data, uint8_t* dst, uint32_t cap);
typedef void (*x07_rt_ext_io_reader_drop_fn_t)(uint32_t data);

extern uint32_t x07_rt_register_io_reader_vtable_v1(
  x07_rt_ext_io_reader_read_fn_t read,
  x07_rt_ext_io_reader_drop_fn_t drop
);

static x07NetCapsV1 g_sock_caps[X07_EXT_SOCKETS_MAX_SOCKS + 1];

static uint32_t x07_ext_sockets_io_reader_read(uint32_t data, uint8_t* dst, uint32_t cap);
static void x07_ext_sockets_io_reader_drop(uint32_t data);

static uint32_t g_io_reader_vtable = 0;

static uint32_t x07_ext_sockets_io_reader_vtable(void) {
  if (g_io_reader_vtable) return g_io_reader_vtable;
  g_io_reader_vtable = x07_rt_register_io_reader_vtable_v1(
    x07_ext_sockets_io_reader_read,
    x07_ext_sockets_io_reader_drop
  );
  return g_io_reader_vtable;
}

static uint32_t x07_ext_sock_caps_get(uint32_t h, x07NetCapsV1* out) {
  if (!out) return 0;
  if (h > 0 && h <= X07_EXT_SOCKETS_MAX_SOCKS && g_sock_caps_ready[h]) {
    *out = g_sock_caps[h];
    return 1;
  }
  return x07_ext_parse_caps_v1(NULL, 0, out);
}

static void x07_ext_sock_caps_set(uint32_t h, const x07NetCapsV1* c) {
  if (!c) return;
  if (h == 0 || h > X07_EXT_SOCKETS_MAX_SOCKS) return;
  g_sock_caps[h] = *c;
  g_sock_caps_ready[h] = 1;
}

static uint32_t x07_ext_sockaddr_to_netaddr_v1_in_place(const struct sockaddr* sa, uint8_t* out, uint32_t out_cap) {
  if (!sa || !out) return 0;
  if (sa->sa_family == AF_INET) {
    if (out_cap < x07_ext_netaddr_ipv4_len()) return 0;
    const struct sockaddr_in* in = (const struct sockaddr_in*)sa;
    uint8_t ip[4];
    memcpy(ip, &in->sin_addr, 4);
    uint32_t port = (uint32_t)ntohs(in->sin_port);
    x07_ext_build_netaddr_ipv4(out, ip, port);
    return x07_ext_netaddr_ipv4_len();
  }
  if (sa->sa_family == AF_INET6) {
    if (out_cap < x07_ext_netaddr_ipv6_len()) return 0;
    const struct sockaddr_in6* in6 = (const struct sockaddr_in6*)sa;
    uint8_t ip[16];
    memcpy(ip, &in6->sin6_addr, 16);
    uint32_t port = (uint32_t)ntohs(in6->sin6_port);
    x07_ext_build_netaddr_ipv6(out, ip, port);
    return x07_ext_netaddr_ipv6_len();
  }
  return 0;
}

static uint32_t x07_ext_sockets_tcp_stream_read_into(
  x07SockEntry* e,
  uint32_t max_bytes,
  uint32_t timeout_ms,
  uint8_t* dst
) {
  if (!e || e->closed) return 0;
  if (max_bytes == 0) return 0;

  for (;;) {
    int revents = 0;
    int prc = x07_ext_poll_one(e->fd, POLLIN, (int)timeout_ms, &revents);
    if (prc == 0) {
      x07_ext_sockets_sock_close_in_place(e);
      return 0;
    }
    if (prc < 0) {
      x07_ext_sockets_sock_close_in_place(e);
      return 0;
    }

    ssize_t got = recv(e->fd, dst, (size_t)max_bytes, 0);
    if (got == 0) {
      x07_ext_sockets_sock_close_in_place(e);
      return 0;
    }
    if (got < 0) {
      int err = x07_sock_errno();
      if (err == EINTR) continue;
      if (err == EAGAIN || err == EWOULDBLOCK) continue;
      x07_ext_sockets_sock_close_in_place(e);
      return 0;
    }
    return (got > 0) ? (uint32_t)got : 0u;
  }
}

static uint32_t x07_ext_sockets_tls_stream_read_into(
  uint32_t stream_handle,
  x07SockEntry* e,
  uint32_t max_bytes,
  uint32_t timeout_ms,
  uint8_t* dst
) {
  if (!e || e->closed) return 0;
  if (max_bytes == 0) return 0;

  SSL* ssl = g_tls[stream_handle].ssl;
  if (!ssl) {
    x07_ext_sockets_sock_close_in_place(e);
    return 0;
  }

  for (;;) {
    int n = SSL_read(ssl, (void*)dst, (int)max_bytes);
    if (n > 0) return (uint32_t)n;

    int err = SSL_get_error(ssl, n);
    if (err == SSL_ERROR_ZERO_RETURN) {
      x07_ext_sockets_sock_close_in_place(e);
      return 0;
    }
    if (err == SSL_ERROR_WANT_READ) {
      int revents = 0;
      int prc = x07_ext_poll_one(e->fd, POLLIN, (int)timeout_ms, &revents);
      if (prc == 0) {
        x07_ext_sockets_sock_close_in_place(e);
        return 0;
      }
      if (prc < 0) {
        x07_ext_sockets_sock_close_in_place(e);
        return 0;
      }
      continue;
    }
    if (err == SSL_ERROR_WANT_WRITE) {
      int revents = 0;
      int prc = x07_ext_poll_one(e->fd, POLLOUT, (int)timeout_ms, &revents);
      if (prc == 0) {
        x07_ext_sockets_sock_close_in_place(e);
        return 0;
      }
      if (prc < 0) {
        x07_ext_sockets_sock_close_in_place(e);
        return 0;
      }
      continue;
    }

    x07_ext_sockets_sock_close_in_place(e);
    return 0;
  }
}

static uint32_t x07_ext_sockets_write_err_doc_v1(uint32_t code, uint8_t* dst, uint32_t cap) {
  if (!dst || cap < 9u) return 0;
  dst[0] = 0;
  x07_ext_write_u32_le(dst + 1, code);
  x07_ext_write_u32_le(dst + 5, 0u);
  return 9u;
}

static uint32_t x07_ext_sockets_udp_recv_doc_into(
  x07SockEntry* e,
  uint32_t max_payload_bytes,
  uint32_t timeout_ms,
  uint8_t* dst,
  uint32_t cap
) {
  if (!e || e->closed) return 0;
  if (!dst) return 0;
  if (cap < 40u) return x07_ext_sockets_write_err_doc_v1(X07_NET_ERR_TOO_LARGE, dst, cap);

  for (;;) {
    int revents = 0;
    int prc = x07_ext_poll_one(e->fd, POLLIN, (int)timeout_ms, &revents);
    if (prc == 0) return x07_ext_sockets_write_err_doc_v1(X07_NET_ERR_TIMEOUT, dst, cap);
    if (prc < 0) return x07_ext_sockets_write_err_doc_v1(X07_NET_ERR_INTERNAL, dst, cap);

    const uint32_t hdr_max = 40u;
    uint32_t payload_cap = cap - hdr_max;
    if (max_payload_bytes < payload_cap) payload_cap = max_payload_bytes;
    if (payload_cap > (uint32_t)INT_MAX) payload_cap = (uint32_t)INT_MAX;

    struct sockaddr_storage ss;
    memset(&ss, 0, sizeof(ss));
    x07_socklen_t ss_len = (x07_socklen_t)sizeof(ss);

    ssize_t got = recvfrom(e->fd, dst + hdr_max, (size_t)payload_cap, 0, (struct sockaddr*)&ss, &ss_len);
    if (got < 0) {
      int err = x07_sock_errno();
      if (err == EINTR) continue;
      if (err == EAGAIN || err == EWOULDBLOCK) return x07_ext_sockets_write_err_doc_v1(X07_NET_ERR_TIMEOUT, dst, cap);
      return x07_ext_sockets_write_err_doc_v1(X07_NET_ERR_CONNECT, dst, cap);
    }
    uint32_t payload_len = (got > 0) ? (uint32_t)got : 0u;

    uint32_t addr_len = x07_ext_sockaddr_to_netaddr_v1_in_place((const struct sockaddr*)&ss, dst + 8, 28u);
    if (addr_len == 0) return x07_ext_sockets_write_err_doc_v1(X07_NET_ERR_INTERNAL, dst, cap);

    uint32_t payload_off = 12u + addr_len;
    if (payload_len != 0 && payload_off != hdr_max) {
      memmove(dst + payload_off, dst + hdr_max, payload_len);
    }

    dst[0] = 1;
    dst[1] = 1;
    dst[2] = 0;
    dst[3] = 0;
    x07_ext_write_u32_le(dst + 4, addr_len);
    x07_ext_write_u32_le(dst + 8 + addr_len, payload_len);
    return payload_off + payload_len;
  }
}

static uint32_t x07_ext_sockets_io_reader_read(uint32_t data, uint8_t* dst, uint32_t cap) {
  if (!x07_ext_wsa_ready()) return 0;
  if (data == 0 || data > X07_EXT_SOCKETS_MAX_SOCKS) return 0;

  x07SockEntry* e = x07_ext_sockets_sock_ptr(data);
  if (!e || e->closed) return 0;

  x07NetCapsV1 c;
  if (!x07_ext_sock_caps_get(data, &c)) return 0;

  if (e->kind == X07_SOCK_KIND_TCP_STREAM) {
    uint32_t max_bytes = cap;
    if (max_bytes > c.max_read_bytes) max_bytes = c.max_read_bytes;
    if (max_bytes > (uint32_t)INT_MAX) max_bytes = (uint32_t)INT_MAX;
    return x07_ext_sockets_tcp_stream_read_into(e, max_bytes, c.io_timeout_ms, dst);
  }
  if (e->kind == X07_SOCK_KIND_TLS_STREAM) {
    uint32_t max_bytes = cap;
    if (max_bytes > c.max_read_bytes) max_bytes = c.max_read_bytes;
    if (max_bytes > (uint32_t)INT_MAX) max_bytes = (uint32_t)INT_MAX;
    return x07_ext_sockets_tls_stream_read_into(data, e, max_bytes, c.io_timeout_ms, dst);
  }
  if (e->kind == X07_SOCK_KIND_UDP) {
    uint32_t max_payload_bytes = cap > 40u ? (cap - 40u) : 0u;
    if (max_payload_bytes > c.max_read_bytes) max_payload_bytes = c.max_read_bytes;
    return x07_ext_sockets_udp_recv_doc_into(e, max_payload_bytes, c.io_timeout_ms, dst, cap);
  }
  return 0;
}

static void x07_ext_sockets_io_reader_drop(uint32_t data) {
  (void)data;
}

int32_t x07_ext_sockets_tcp_stream_reader_prepare_v1(uint32_t stream_handle, const uint8_t* caps, uint32_t caps_len) {
  if (!x07_ext_wsa_ready()) return 0;
  uint32_t vtable = x07_ext_sockets_io_reader_vtable();
  if (vtable == 0 || vtable > (uint32_t)INT32_MAX) return 0;

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->closed) return 0;
  if (e->kind != X07_SOCK_KIND_TCP_STREAM && e->kind != X07_SOCK_KIND_TLS_STREAM) return 0;

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) {
    (void)x07_ext_parse_caps_v1(NULL, 0, &c);
  }
  x07_ext_sock_caps_set(stream_handle, &c);
  return (int32_t)vtable;
}

int32_t x07_ext_sockets_udp_recv_doc_reader_prepare_v1(uint32_t sock_handle, const uint8_t* caps, uint32_t caps_len) {
  if (!x07_ext_wsa_ready()) return 0;
  uint32_t vtable = x07_ext_sockets_io_reader_vtable();
  if (vtable == 0 || vtable > (uint32_t)INT32_MAX) return 0;

  x07SockEntry* e = x07_ext_sockets_sock_ptr(sock_handle);
  if (!e || e->closed) return 0;
  if (e->kind != X07_SOCK_KIND_UDP) return 0;

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) {
    (void)x07_ext_parse_caps_v1(NULL, 0, &c);
  }
  x07_ext_sock_caps_set(sock_handle, &c);
  return (int32_t)vtable;
}

static int x07_ext_parse_tls_client_cfg_v1(
  const uint8_t* cfg,
  uint32_t cfg_len,
  const x07NetAddrV1* addr,
  x07TlsClientCfgV1* out
) {
  if (!out) return 0;
  out->verify_peer = 1;
  out->sni = NULL;
  out->sni_len = 0;

  if (!cfg || cfg_len == 0) {
    if (addr && addr->tag == X07_NET_ADDR_TAG_DNS) {
      out->sni = addr->dns;
      out->sni_len = addr->dns_len;
    }
    if (out->verify_peer && out->sni_len == 0) return 0;
    return 1;
  }

  if (cfg_len < 16u) return 0;
  uint32_t pos = 0;
  uint32_t ver = 0;
  uint32_t verify_peer = 0;
  uint32_t sni_len = 0;
  uint32_t reserved = 0;

  if (!x07_ext_read_u32_le(cfg, cfg_len, &pos, &ver)) return 0;
  if (ver != 1u) return 0;
  if (!x07_ext_read_u32_le(cfg, cfg_len, &pos, &verify_peer)) return 0;
  if (verify_peer > 1u) return 0;
  if (!x07_ext_read_u32_le(cfg, cfg_len, &pos, &sni_len)) return 0;
  if (sni_len > 1024u) return 0;
  if (pos > cfg_len || cfg_len - pos < sni_len + 4u) return 0;

  const uint8_t* sni = cfg + pos;
  pos += sni_len;
  if (!x07_ext_read_u32_le(cfg, cfg_len, &pos, &reserved)) return 0;
  if (pos != cfg_len) return 0;
  if (reserved != 0) return 0;

  if (sni_len) {
    if (memchr(sni, 0, sni_len) != NULL) return 0;
    out->sni = sni;
    out->sni_len = sni_len;
  } else {
    if (addr && addr->tag == X07_NET_ADDR_TAG_DNS) {
      out->sni = addr->dns;
      out->sni_len = addr->dns_len;
    }
  }
  out->verify_peer = verify_peer;
  if (out->verify_peer && out->sni_len == 0) return 0;
  return 1;
}

static int x07_ext_parse_addr_v1(const uint8_t* b, uint32_t n, x07NetAddrV1* out) {
  if (!b || !out) return 0;
  memset(out, 0, sizeof(*out));

  uint32_t pos = 0;
  uint32_t ver = 0;
  if (!x07_ext_read_u32_le(b, n, &pos, &ver)) return 0;
  if (ver != 1u) return 0;
  uint32_t tag = 0;
  if (!x07_ext_read_u32_le(b, n, &pos, &tag)) return 0;
  uint32_t port = 0;
  if (!x07_ext_read_u32_le(b, n, &pos, &port)) return 0;
  if (port > 65535u) return 0;

  out->tag = tag;
  out->port = port;

  if (tag == X07_NET_ADDR_TAG_IPV4) {
    if (pos > n || n - pos != 4u) return 0;
    out->ip = b + pos;
    out->ip_len = 4u;
    return 1;
  }
  if (tag == X07_NET_ADDR_TAG_IPV6) {
    if (pos > n || n - pos != 16u) return 0;
    out->ip = b + pos;
    out->ip_len = 16u;
    return 1;
  }
  if (tag == X07_NET_ADDR_TAG_DNS) {
    uint32_t name_len = 0;
    if (!x07_ext_read_u32_le(b, n, &pos, &name_len)) return 0;
    if (pos > n || name_len > n - pos) return 0;
    if (name_len == 0) return 0;
    const uint8_t* name = b + pos;
    // Reject NUL bytes so we can safely treat it as a string boundary when needed.
    if (memchr(name, 0, name_len) != NULL) return 0;
    out->dns = name;
    out->dns_len = name_len;
    pos += name_len;
    if (pos != n) return 0;
    return 1;
  }

  return 0;
}

static uint32_t x07_ext_netaddr_ipv4_len(void) { return 4u + 4u + 4u + 4u; }
static uint32_t x07_ext_netaddr_ipv6_len(void) { return 4u + 4u + 4u + 16u; }

static void x07_ext_build_netaddr_ipv4(uint8_t* out, const uint8_t ip[4], uint32_t port) {
  x07_ext_write_u32_le(out + 0, 1u);
  x07_ext_write_u32_le(out + 4, X07_NET_ADDR_TAG_IPV4);
  x07_ext_write_u32_le(out + 8, port);
  memcpy(out + 12, ip, 4);
}

static void x07_ext_build_netaddr_ipv6(uint8_t* out, const uint8_t ip[16], uint32_t port) {
  x07_ext_write_u32_le(out + 0, 1u);
  x07_ext_write_u32_le(out + 4, X07_NET_ADDR_TAG_IPV6);
  x07_ext_write_u32_le(out + 8, port);
  memcpy(out + 12, ip, 16);
}

static int x07_ext_wsa_ready(void) { return 1; }

static int x07_ext_sock_set_nonblocking(x07_sock_t fd) {
  int flags = fcntl(fd, F_GETFL, 0);
  if (flags < 0) return 0;
  if (fcntl(fd, F_SETFL, flags | O_NONBLOCK) != 0) return 0;
  return 1;
}

static int x07_ext_poll_one(x07_sock_t fd, int events, int timeout_ms, int* out_revents) {
  if (out_revents) *out_revents = 0;
  struct pollfd pfd;
  memset(&pfd, 0, sizeof(pfd));
  pfd.fd = fd;
  pfd.events = (short)events;
  int rc = poll(&pfd, 1, timeout_ms);
  if (rc <= 0) return rc;
  if (out_revents) *out_revents = (int)pfd.revents;
  return rc;
}

static int x07_ext_sock_get_so_error(x07_sock_t fd, int* out_err) {
  if (out_err) *out_err = 0;
  int so_error = 0;
  x07_socklen_t len = (x07_socklen_t)sizeof(so_error);
  if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &so_error, &len) != 0) return 0;
  if (out_err) *out_err = so_error;
  return 1;
}

static uint32_t x07_ext_tcp_connect_addr(const struct sockaddr* sa, x07_socklen_t sa_len, uint32_t timeout_ms, x07_sock_t* out_fd) {
  if (out_fd) *out_fd = X07_SOCK_INVALID;
  if (!sa || sa_len == 0) return X07_NET_ERR_CONNECT;

  x07_sock_t fd = socket(sa->sa_family, SOCK_STREAM, IPPROTO_TCP);
  if (fd == X07_SOCK_INVALID) return X07_NET_ERR_CONNECT;
  if (!x07_ext_sock_set_nonblocking(fd)) {
    (void)x07_sock_close(fd);
    return X07_NET_ERR_INTERNAL;
  }

  int rc = connect(fd, sa, (x07_socklen_t)sa_len);
  if (rc != 0) {
    int err = x07_sock_errno();
    if (err != EINPROGRESS) {
      (void)x07_sock_close(fd);
      return X07_NET_ERR_CONNECT;
    }
  }

  int revents = 0;
  int prc = x07_ext_poll_one(fd, POLLOUT, (int)timeout_ms, &revents);
  if (prc == 0) {
    (void)x07_sock_close(fd);
    return X07_NET_ERR_TIMEOUT;
  }
  if (prc < 0) {
    (void)x07_sock_close(fd);
    return X07_NET_ERR_INTERNAL;
  }

  int so_error = 0;
  if (!x07_ext_sock_get_so_error(fd, &so_error) || so_error != 0) {
    (void)x07_sock_close(fd);
    return X07_NET_ERR_CONNECT;
  }

  if (out_fd) *out_fd = fd;
  return 0;
}

static const char* x07_ext_inet_ntop(int family, const void* addr, char* out, size_t out_sz);
static int x07_ext_addr_policy_check_sandboxed(const x07NetAddrV1* addr, uint32_t op_port);

static uint32_t x07_ext_tcp_connect_fd_v1(const x07NetAddrV1* a, const x07NetCapsV1* c, x07_sock_t* out_fd) {
  if (out_fd) *out_fd = X07_SOCK_INVALID;
  if (!a || !c || !out_fd) return X07_NET_ERR_INTERNAL;

  const char* allow_hosts = NULL;
  int sandboxed = x07_ext_env_is_1("X07_OS_SANDBOXED");
  if (sandboxed) {
    if (!x07_ext_env_is_1("X07_OS_NET")) return X07_NET_ERR_POLICY_DENIED;
    if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_TCP")) return X07_NET_ERR_POLICY_DENIED;
    allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
    if (!allow_hosts || !*allow_hosts) return X07_NET_ERR_POLICY_DENIED;

    if (a->tag == X07_NET_ADDR_TAG_DNS) {
      if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_DNS")) return X07_NET_ERR_POLICY_DENIED;
      if (!x07_ext_host_port_allowed(allow_hosts, a->dns, (size_t)a->dns_len, a->port)) return X07_NET_ERR_POLICY_DENIED;
    } else {
      if (!x07_ext_addr_policy_check_sandboxed(a, a->port)) return X07_NET_ERR_POLICY_DENIED;
    }
  }

  struct sockaddr_storage ss;
  x07_socklen_t ss_len = 0;
  memset(&ss, 0, sizeof(ss));

  x07_sock_t fd = X07_SOCK_INVALID;
  uint32_t saw_timeout = 0;
  uint32_t saw_internal = 0;
  uint32_t saw_policy_allowed_addr = 0;

  if (a->tag == X07_NET_ADDR_TAG_DNS) {
    char host[1025];
    if (a->dns_len > 1024u) return X07_NET_ERR_INVALID_REQ;
    memcpy(host, a->dns, (size_t)a->dns_len);
    host[a->dns_len] = '\0';

    char service[16];
    snprintf(service, sizeof(service), "%u", (unsigned)a->port);

    struct addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_protocol = IPPROTO_TCP;
    struct addrinfo* res = NULL;
    int gai = getaddrinfo(host, service, &hints, &res);
    if (gai != 0 || !res) return X07_NET_ERR_DNS;

    for (struct addrinfo* it = res; it; it = it->ai_next) {
      if (!it->ai_addr || it->ai_addrlen == 0 || it->ai_addrlen > (x07_socklen_t)sizeof(ss)) continue;

      if (sandboxed) {
        char host_buf[INET6_ADDRSTRLEN];
        memset(host_buf, 0, sizeof(host_buf));
        if (it->ai_addr->sa_family == AF_INET) {
          const struct sockaddr_in* in = (const struct sockaddr_in*)it->ai_addr;
          const char* s = x07_ext_inet_ntop(AF_INET, &in->sin_addr, host_buf, sizeof(host_buf));
          if (!s) continue;
        } else if (it->ai_addr->sa_family == AF_INET6) {
          const struct sockaddr_in6* in6 = (const struct sockaddr_in6*)it->ai_addr;
          const char* s = x07_ext_inet_ntop(AF_INET6, &in6->sin6_addr, host_buf, sizeof(host_buf));
          if (!s) continue;
        } else {
          continue;
        }
        if (!x07_ext_host_port_allowed(allow_hosts, (const uint8_t*)host_buf, strlen(host_buf), a->port)) continue;
      }

      saw_policy_allowed_addr = 1;
      uint32_t err = x07_ext_tcp_connect_addr(it->ai_addr, (x07_socklen_t)it->ai_addrlen, c->connect_timeout_ms, &fd);
      if (err == 0) break;
      if (err == X07_NET_ERR_TIMEOUT) saw_timeout = 1;
      if (err == X07_NET_ERR_INTERNAL) saw_internal = 1;
      fd = X07_SOCK_INVALID;
    }
    freeaddrinfo(res);

    if (fd == X07_SOCK_INVALID) {
      if (sandboxed && !saw_policy_allowed_addr) return X07_NET_ERR_POLICY_DENIED;
      if (saw_timeout) return X07_NET_ERR_TIMEOUT;
      if (saw_internal) return X07_NET_ERR_INTERNAL;
      return X07_NET_ERR_CONNECT;
    }
  } else if (a->tag == X07_NET_ADDR_TAG_IPV4) {
    struct sockaddr_in in;
    memset(&in, 0, sizeof(in));
    in.sin_family = AF_INET;
    in.sin_port = htons((uint16_t)a->port);
    memcpy(&in.sin_addr, a->ip, 4);
    memcpy(&ss, &in, sizeof(in));
    ss_len = (x07_socklen_t)sizeof(in);
  } else if (a->tag == X07_NET_ADDR_TAG_IPV6) {
    struct sockaddr_in6 in6;
    memset(&in6, 0, sizeof(in6));
    in6.sin6_family = AF_INET6;
    in6.sin6_port = htons((uint16_t)a->port);
    memcpy(&in6.sin6_addr, a->ip, 16);
    memcpy(&ss, &in6, sizeof(in6));
    ss_len = (x07_socklen_t)sizeof(in6);
  } else {
    return X07_NET_ERR_INVALID_REQ;
  }

  if (a->tag != X07_NET_ADDR_TAG_DNS) {
    uint32_t err = x07_ext_tcp_connect_addr((struct sockaddr*)&ss, ss_len, c->connect_timeout_ms, &fd);
    if (err != 0) return err;
  }

  *out_fd = fd;
  return 0;
}

static const char* x07_ext_inet_ntop(int family, const void* addr, char* out, size_t out_sz) {
  return inet_ntop(family, addr, out, (socklen_t)out_sz);
}

static uint32_t x07_ext_tls_client_handshake(
  x07_sock_t fd,
  const x07TlsClientCfgV1* cfg,
  uint32_t timeout_ms,
  SSL_CTX** out_ctx,
  SSL** out_ssl
) {
  if (out_ctx) *out_ctx = NULL;
  if (out_ssl) *out_ssl = NULL;
  if (!cfg || !out_ctx || !out_ssl) return X07_NET_ERR_INTERNAL;

  SSL_CTX* ctx = SSL_CTX_new(TLS_client_method());
  if (!ctx) return X07_NET_ERR_INTERNAL;

  // Disable legacy protocols unconditionally.
  (void)SSL_CTX_set_options(ctx, SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3);

  if (cfg->verify_peer) {
    SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, NULL);
    if (SSL_CTX_set_default_verify_paths(ctx) != 1) {
      SSL_CTX_free(ctx);
      return X07_NET_ERR_TLS;
    }
  } else {
    SSL_CTX_set_verify(ctx, SSL_VERIFY_NONE, NULL);
  }

  SSL* ssl = SSL_new(ctx);
  if (!ssl) {
    SSL_CTX_free(ctx);
    return X07_NET_ERR_INTERNAL;
  }

  if (cfg->sni && cfg->sni_len) {
    char sni_raw[1025];
    if (cfg->sni_len > 1024u) {
      SSL_free(ssl);
      SSL_CTX_free(ctx);
      return X07_NET_ERR_INVALID_REQ;
    }
    memcpy(sni_raw, cfg->sni, (size_t)cfg->sni_len);
    sni_raw[cfg->sni_len] = '\0';

    const char* sni = sni_raw;
    size_t sni_len = (size_t)cfg->sni_len;
    if (sni_len >= 2 && sni[0] == '[' && sni[sni_len - 1] == ']') {
      sni += 1;
      sni_len -= 2;
    }

    int sni_is_ip = 0;
    {
      int fam = 0;
      uint8_t ip[16];
      memset(ip, 0, sizeof(ip));
      sni_is_ip = x07_ext_parse_ip_bytes((const uint8_t*)sni, sni_len, &fam, ip);
    }

    if (!sni_is_ip) {
      if (SSL_set_tlsext_host_name(ssl, sni) != 1) {
        SSL_free(ssl);
        SSL_CTX_free(ctx);
        return X07_NET_ERR_TLS;
      }
      if (cfg->verify_peer) {
        X509_VERIFY_PARAM* param = SSL_get0_param(ssl);
        (void)X509_VERIFY_PARAM_set_hostflags(param, X509_CHECK_FLAG_NO_PARTIAL_WILDCARDS);
        if (X509_VERIFY_PARAM_set1_host(param, sni, 0) != 1) {
          SSL_free(ssl);
          SSL_CTX_free(ctx);
          return X07_NET_ERR_TLS;
        }
      }
    } else {
      if (cfg->verify_peer) {
        char ip_str[1025];
        if (sni_len > 1024u) {
          SSL_free(ssl);
          SSL_CTX_free(ctx);
          return X07_NET_ERR_INVALID_REQ;
        }
        memcpy(ip_str, sni, sni_len);
        ip_str[sni_len] = '\0';

        X509_VERIFY_PARAM* param = SSL_get0_param(ssl);
        if (X509_VERIFY_PARAM_set1_ip_asc(param, ip_str) != 1) {
          SSL_free(ssl);
          SSL_CTX_free(ctx);
          return X07_NET_ERR_TLS;
        }
      }
    }
  }

  if (SSL_set_fd(ssl, fd) != 1) {
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return X07_NET_ERR_TLS;
  }

  for (;;) {
    ERR_clear_error();
    int rc = SSL_connect(ssl);
    if (rc == 1) break;

    int err = SSL_get_error(ssl, rc);
    if (err == SSL_ERROR_WANT_READ) {
      int revents = 0;
      int prc = x07_ext_poll_one(fd, POLLIN, (int)timeout_ms, &revents);
      if (prc == 0) {
        SSL_free(ssl);
        SSL_CTX_free(ctx);
        return X07_NET_ERR_TIMEOUT;
      }
      if (prc < 0) {
        SSL_free(ssl);
        SSL_CTX_free(ctx);
        return X07_NET_ERR_INTERNAL;
      }
      continue;
    }
    if (err == SSL_ERROR_WANT_WRITE) {
      int revents = 0;
      int prc = x07_ext_poll_one(fd, POLLOUT, (int)timeout_ms, &revents);
      if (prc == 0) {
        SSL_free(ssl);
        SSL_CTX_free(ctx);
        return X07_NET_ERR_TIMEOUT;
      }
      if (prc < 0) {
        SSL_free(ssl);
        SSL_CTX_free(ctx);
        return X07_NET_ERR_INTERNAL;
      }
      continue;
    }

    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return X07_NET_ERR_TLS;
  }

  *out_ctx = ctx;
  *out_ssl = ssl;
  return 0;
}

static int x07_ext_sockaddr_to_netaddr_v1(const struct sockaddr* sa, uint32_t port_override, uint8_t** out_bytes, uint32_t* out_len) {
  if (out_bytes) *out_bytes = NULL;
  if (out_len) *out_len = 0;
  if (!sa) return 0;

  if (sa->sa_family == AF_INET) {
    const struct sockaddr_in* in = (const struct sockaddr_in*)sa;
    uint8_t ip[4];
    memcpy(ip, &in->sin_addr, 4);
    uint32_t port = port_override ? port_override : (uint32_t)ntohs(in->sin_port);
    uint32_t len = x07_ext_netaddr_ipv4_len();
    uint8_t* b = (uint8_t*)malloc((size_t)len);
    if (!b) return 0;
    x07_ext_build_netaddr_ipv4(b, ip, port);
    if (out_bytes) *out_bytes = b;
    if (out_len) *out_len = len;
    return 1;
  }
  if (sa->sa_family == AF_INET6) {
    const struct sockaddr_in6* in6 = (const struct sockaddr_in6*)sa;
    uint8_t ip[16];
    memcpy(ip, &in6->sin6_addr, 16);
    uint32_t port = port_override ? port_override : (uint32_t)ntohs(in6->sin6_port);
    uint32_t len = x07_ext_netaddr_ipv6_len();
    uint8_t* b = (uint8_t*)malloc((size_t)len);
    if (!b) return 0;
    x07_ext_build_netaddr_ipv6(b, ip, port);
    if (out_bytes) *out_bytes = b;
    if (out_len) *out_len = len;
    return 1;
  }
  return 0;
}

static int x07_ext_addr_policy_check_sandboxed(const x07NetAddrV1* addr, uint32_t op_port) {
  if (!addr) return 0;
  if (!x07_ext_env_is_1("X07_OS_SANDBOXED")) return 1;

  if (!x07_ext_env_is_1("X07_OS_NET")) return 0;
  const char* allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
  if (!allow_hosts || !*allow_hosts) return 0;

  uint32_t port = op_port ? op_port : addr->port;
  if (port == 0) return 0;

  if (addr->tag == X07_NET_ADDR_TAG_DNS) {
    if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_DNS")) return 0;
    return x07_ext_host_port_allowed(allow_hosts, addr->dns, (size_t)addr->dns_len, port);
  }

  char host_buf[INET6_ADDRSTRLEN];
  memset(host_buf, 0, sizeof(host_buf));
  if (addr->tag == X07_NET_ADDR_TAG_IPV4) {
    struct in_addr a;
    memcpy(&a, addr->ip, 4);
    const char* s = x07_ext_inet_ntop(AF_INET, &a, host_buf, sizeof(host_buf));
    if (!s) return 0;
    return x07_ext_host_port_allowed(allow_hosts, (const uint8_t*)host_buf, strlen(host_buf), port);
  }
  if (addr->tag == X07_NET_ADDR_TAG_IPV6) {
    struct in6_addr a6;
    memcpy(&a6, addr->ip, 16);
    const char* s = x07_ext_inet_ntop(AF_INET6, &a6, host_buf, sizeof(host_buf));
    if (!s) return 0;
    return x07_ext_host_port_allowed(allow_hosts, (const uint8_t*)host_buf, strlen(host_buf), port);
  }
  return 0;
}

// ---- Doc builders ----

static uint8_t* x07_ext_make_ok_handle_doc(uint32_t handle, uint32_t* out_len) {
  uint8_t* buf = (uint8_t*)malloc(8);
  if (!buf) return NULL;
  buf[0] = 1;
  buf[1] = 1;
  buf[2] = 0;
  buf[3] = 0;
  x07_ext_write_u32_le(buf + 4, handle);
  if (out_len) *out_len = 8;
  return buf;
}

static uint8_t* x07_ext_make_ok_wait_doc(uint32_t events, uint32_t* out_len) {
  uint8_t* buf = (uint8_t*)malloc(8);
  if (!buf) return NULL;
  buf[0] = 1;
  buf[1] = 1;
  buf[2] = 0;
  buf[3] = 0;
  x07_ext_write_u32_le(buf + 4, events);
  if (out_len) *out_len = 8;
  return buf;
}

static uint8_t* x07_ext_make_ok_write_doc(uint32_t bytes_written, uint32_t* out_len) {
  return x07_ext_make_ok_wait_doc(bytes_written, out_len);
}

static uint8_t* x07_ext_make_ok_read_doc(const uint8_t* payload, uint32_t payload_len, uint32_t* out_len) {
  uint64_t doc_len = 8ull + (uint64_t)payload_len;
  if (doc_len > (uint64_t)UINT32_MAX) return NULL;
  uint8_t* buf = (uint8_t*)malloc((size_t)doc_len);
  if (!buf) return NULL;
  buf[0] = 1;
  buf[1] = 1;
  buf[2] = 0;
  buf[3] = 0;
  x07_ext_write_u32_le(buf + 4, payload_len);
  if (payload_len && payload) memcpy(buf + 8, payload, (size_t)payload_len);
  if (out_len) *out_len = (uint32_t)doc_len;
  return buf;
}

static uint8_t* x07_ext_make_ok_handle_addr_doc(uint32_t handle, const uint8_t* addr, uint32_t addr_len, uint32_t* out_len) {
  uint64_t doc_len = 12ull + (uint64_t)addr_len;
  if (doc_len > (uint64_t)UINT32_MAX) return NULL;
  uint8_t* buf = (uint8_t*)malloc((size_t)doc_len);
  if (!buf) return NULL;
  buf[0] = 1;
  buf[1] = 1;
  buf[2] = 0;
  buf[3] = 0;
  x07_ext_write_u32_le(buf + 4, handle);
  x07_ext_write_u32_le(buf + 8, addr_len);
  if (addr_len && addr) memcpy(buf + 12, addr, (size_t)addr_len);
  if (out_len) *out_len = (uint32_t)doc_len;
  return buf;
}

static uint8_t* x07_ext_make_ok_handle_peer_doc(uint32_t handle, const uint8_t* peer_addr, uint32_t peer_addr_len, uint32_t* out_len) {
  return x07_ext_make_ok_handle_addr_doc(handle, peer_addr, peer_addr_len, out_len);
}

static uint8_t* x07_ext_make_ok_udp_recv_doc(const uint8_t* from_addr, uint32_t from_addr_len, const uint8_t* payload, uint32_t payload_len, uint32_t* out_len) {
  uint64_t doc_len = 12ull + (uint64_t)from_addr_len + (uint64_t)payload_len;
  if (doc_len > (uint64_t)UINT32_MAX) return NULL;
  uint8_t* buf = (uint8_t*)malloc((size_t)doc_len);
  if (!buf) return NULL;
  uint32_t p = 0;
  buf[p++] = 1;
  buf[p++] = 1;
  buf[p++] = 0;
  buf[p++] = 0;
  x07_ext_write_u32_le(buf + p, from_addr_len);
  p += 4;
  if (from_addr_len && from_addr) memcpy(buf + p, from_addr, (size_t)from_addr_len);
  p += from_addr_len;
  x07_ext_write_u32_le(buf + p, payload_len);
  p += 4;
  if (payload_len && payload) memcpy(buf + p, payload, (size_t)payload_len);
  p += payload_len;
  if (out_len) *out_len = p;
  return buf;
}

static uint8_t* x07_ext_make_ok_dns_doc(uint32_t count, const uint8_t* addrs_blob, uint32_t addrs_blob_len, uint32_t* out_len) {
  uint64_t doc_len = 8ull + (uint64_t)addrs_blob_len;
  if (doc_len > (uint64_t)UINT32_MAX) return NULL;
  uint8_t* buf = (uint8_t*)malloc((size_t)doc_len);
  if (!buf) return NULL;
  buf[0] = 1;
  buf[1] = 1;
  buf[2] = 0;
  buf[3] = 0;
  x07_ext_write_u32_le(buf + 4, count);
  if (addrs_blob_len && addrs_blob) memcpy(buf + 8, addrs_blob, (size_t)addrs_blob_len);
  if (out_len) *out_len = (uint32_t)doc_len;
  return buf;
}

static int x07_ext_store_doc(uint8_t* doc, uint32_t doc_len, uint32_t* out_handle) {
  if (out_handle) *out_handle = 0;
  if (!doc || doc_len == 0) return 0;
  uint32_t slot = x07_ext_sockets_alloc_buf_slot();
  if (!slot) return 0;
  g_bufs[slot] = doc;
  g_lens[slot] = doc_len;
  if (out_handle) *out_handle = slot;
  return 1;
}

static int32_t x07_ext_return_err(uint32_t code, uint32_t* out_handle) {
  uint32_t len = 0;
  uint8_t* doc = x07_ext_make_err_doc(code, &len);
  if (!doc) return -1;
  if (!x07_ext_store_doc(doc, len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

// ---- Public entrypoints (exported to X07 via extern) ----

int32_t x07_ext_sockets_dns_lookup_alloc(
  const uint8_t* name,
  uint32_t name_len,
  uint32_t port,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (!name || name_len == 0 || name_len > 1024u) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (memchr(name, 0, name_len) != NULL) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (port == 0 || port > 65535u) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  const char* allow_hosts = NULL;
  int sandboxed = x07_ext_env_is_1("X07_OS_SANDBOXED");
  if (sandboxed) {
    if (!x07_ext_env_is_1("X07_OS_NET")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_DNS")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
    if (!allow_hosts || !*allow_hosts) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_host_port_allowed(allow_hosts, name, (size_t)name_len, port)) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
  }

  char host[1025];
  memcpy(host, name, (size_t)name_len);
  host[name_len] = '\0';

  char service[16];
  snprintf(service, sizeof(service), "%u", (unsigned)port);

  struct addrinfo hints;
  memset(&hints, 0, sizeof(hints));
  hints.ai_family = AF_UNSPEC;
  hints.ai_socktype = SOCK_STREAM;
  hints.ai_protocol = IPPROTO_TCP;

  struct addrinfo* res = NULL;
  int gai = getaddrinfo(host, service, &hints, &res);
  if (gai != 0 || !res) return x07_ext_return_err(X07_NET_ERR_DNS, out_handle);

  // Pack as: repeated (addr_len:u32_le + addr_bytes[addr_len]).
  uint8_t addrs_tmp[64 * 64];
  uint32_t addrs_len = 0;
  uint32_t count = 0;

  for (struct addrinfo* it = res; it && count < 64u; it = it->ai_next) {
    if (sandboxed) {
      char host_buf[INET6_ADDRSTRLEN];
      memset(host_buf, 0, sizeof(host_buf));
      if (it->ai_addr && it->ai_addr->sa_family == AF_INET) {
        const struct sockaddr_in* in = (const struct sockaddr_in*)it->ai_addr;
        const char* s = x07_ext_inet_ntop(AF_INET, &in->sin_addr, host_buf, sizeof(host_buf));
        if (!s) continue;
      } else if (it->ai_addr && it->ai_addr->sa_family == AF_INET6) {
        const struct sockaddr_in6* in6 = (const struct sockaddr_in6*)it->ai_addr;
        const char* s = x07_ext_inet_ntop(AF_INET6, &in6->sin6_addr, host_buf, sizeof(host_buf));
        if (!s) continue;
      } else {
        continue;
      }
      if (!x07_ext_host_port_allowed(allow_hosts, (const uint8_t*)host_buf, strlen(host_buf), port)) continue;
    }

    uint8_t* addr_bytes = NULL;
    uint32_t addr_len = 0;
    if (!x07_ext_sockaddr_to_netaddr_v1(it->ai_addr, port, &addr_bytes, &addr_len)) continue;
    if (addr_len == 0 || addr_len > 128u) {
      free(addr_bytes);
      continue;
    }
    if (addrs_len > sizeof(addrs_tmp) || (uint32_t)sizeof(addrs_tmp) - addrs_len < 4u + addr_len) {
      free(addr_bytes);
      break;
    }
    x07_ext_write_u32_le(addrs_tmp + addrs_len, addr_len);
    addrs_len += 4u;
    memcpy(addrs_tmp + addrs_len, addr_bytes, addr_len);
    addrs_len += addr_len;
    free(addr_bytes);
    count += 1;
  }
  freeaddrinfo(res);

  if (sandboxed && count == 0) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_dns_doc(count, addrs_tmp, addrs_len, &doc_len);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tcp_connect_alloc(
  const uint8_t* addr,
  uint32_t addr_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07NetAddrV1 a;
  if (!x07_ext_parse_addr_v1(addr, addr_len, &a)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07_sock_t fd = X07_SOCK_INVALID;
  uint32_t err = x07_ext_tcp_connect_fd_v1(&a, &c, &fd);
  if (err != 0) return x07_ext_return_err(err, out_handle);

  uint32_t stream_handle = x07_ext_sockets_alloc_sock_slot(X07_SOCK_KIND_TCP_STREAM, fd);
  if (!stream_handle) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_handle_doc(stream_handle, &doc_len);
  if (!doc) {
    (void)x07_ext_sockets_sock_drop(stream_handle, X07_SOCK_KIND_TCP_STREAM);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    (void)x07_ext_sockets_sock_drop(stream_handle, X07_SOCK_KIND_TCP_STREAM);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tls_connect_alloc(
  const uint8_t* addr,
  uint32_t addr_len,
  const uint8_t* tls_cfg,
  uint32_t tls_cfg_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07NetAddrV1 a;
  if (!x07_ext_parse_addr_v1(addr, addr_len, &a)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07TlsClientCfgV1 t;
  if (!x07_ext_parse_tls_client_cfg_v1(tls_cfg, tls_cfg_len, &a, &t)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07_sock_t fd = X07_SOCK_INVALID;
  uint32_t err = x07_ext_tcp_connect_fd_v1(&a, &c, &fd);
  if (err != 0) return x07_ext_return_err(err, out_handle);

  SSL_CTX* ctx = NULL;
  SSL* ssl = NULL;
  err = x07_ext_tls_client_handshake(fd, &t, c.connect_timeout_ms, &ctx, &ssl);
  if (err != 0) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(err, out_handle);
  }

  uint32_t stream_handle = x07_ext_sockets_alloc_sock_slot(X07_SOCK_KIND_TLS_STREAM, fd);
  if (!stream_handle) {
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  g_tls[stream_handle].ctx = ctx;
  g_tls[stream_handle].ssl = ssl;

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_handle_doc(stream_handle, &doc_len);
  if (!doc) {
    (void)x07_ext_sockets_sock_drop(stream_handle, X07_SOCK_KIND_TLS_STREAM);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    (void)x07_ext_sockets_sock_drop(stream_handle, X07_SOCK_KIND_TLS_STREAM);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tcp_listen_alloc(
  const uint8_t* addr,
  uint32_t addr_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07NetAddrV1 a;
  if (!x07_ext_parse_addr_v1(addr, addr_len, &a)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (a.tag != X07_NET_ADDR_TAG_IPV4 && a.tag != X07_NET_ADDR_TAG_IPV6) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  if (x07_ext_env_is_1("X07_OS_SANDBOXED")) {
    if (!x07_ext_env_is_1("X07_OS_NET")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_TCP")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_addr_policy_check_sandboxed(&a, a.port)) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
  }

  struct sockaddr_storage ss;
  x07_socklen_t ss_len = 0;
  memset(&ss, 0, sizeof(ss));

  if (a.tag == X07_NET_ADDR_TAG_IPV4) {
    struct sockaddr_in in;
    memset(&in, 0, sizeof(in));
    in.sin_family = AF_INET;
    in.sin_port = htons((uint16_t)a.port);
    memcpy(&in.sin_addr, a.ip, 4);
    memcpy(&ss, &in, sizeof(in));
    ss_len = (x07_socklen_t)sizeof(in);
  } else {
    struct sockaddr_in6 in6;
    memset(&in6, 0, sizeof(in6));
    in6.sin6_family = AF_INET6;
    in6.sin6_port = htons((uint16_t)a.port);
    memcpy(&in6.sin6_addr, a.ip, 16);
    memcpy(&ss, &in6, sizeof(in6));
    ss_len = (x07_socklen_t)sizeof(in6);
  }

  x07_sock_t fd = socket(ss.ss_family, SOCK_STREAM, IPPROTO_TCP);
  if (fd == X07_SOCK_INVALID) return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);

  int opt = 1;
  (void)setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, (x07_socklen_t)sizeof(opt));

  if (bind(fd, (struct sockaddr*)&ss, (x07_socklen_t)ss_len) != 0) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  }
  if (listen(fd, 128) != 0) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  }
  if (!x07_ext_sock_set_nonblocking(fd)) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  // Discover bound addr (port=0).
  struct sockaddr_storage bound;
  x07_socklen_t bound_len = (x07_socklen_t)sizeof(bound);
  memset(&bound, 0, sizeof(bound));
  if (getsockname(fd, (struct sockaddr*)&bound, &bound_len) != 0) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint8_t* bound_addr = NULL;
  uint32_t bound_addr_len = 0;
  if (!x07_ext_sockaddr_to_netaddr_v1((struct sockaddr*)&bound, 0, &bound_addr, &bound_addr_len)) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  if (x07_ext_env_is_1("X07_OS_SANDBOXED")) {
    x07NetAddrV1 ba;
    if (!x07_ext_parse_addr_v1(bound_addr, bound_addr_len, &ba) || !x07_ext_addr_policy_check_sandboxed(&ba, ba.port)) {
      free(bound_addr);
      (void)x07_sock_close(fd);
      return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    }
  }

  uint32_t listener_handle = x07_ext_sockets_alloc_sock_slot(X07_SOCK_KIND_TCP_LISTENER, fd);
  if (!listener_handle) {
    free(bound_addr);
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_handle_addr_doc(listener_handle, bound_addr, bound_addr_len, &doc_len);
  free(bound_addr);
  if (!doc) {
    (void)x07_ext_sockets_sock_drop(listener_handle, X07_SOCK_KIND_TCP_LISTENER);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    (void)x07_ext_sockets_sock_drop(listener_handle, X07_SOCK_KIND_TCP_LISTENER);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tcp_accept_alloc(
  uint32_t listener_handle,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(listener_handle);
  if (!e || e->kind != X07_SOCK_KIND_TCP_LISTENER || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  int revents = 0;
  int prc = x07_ext_poll_one(e->fd, POLLIN, (int)c.io_timeout_ms, &revents);
  if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
  if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  struct sockaddr_storage peer;
  x07_socklen_t peer_len = (x07_socklen_t)sizeof(peer);
  memset(&peer, 0, sizeof(peer));
  x07_sock_t fd = accept(e->fd, (struct sockaddr*)&peer, &peer_len);
  if (fd == X07_SOCK_INVALID) return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  if (!x07_ext_sock_set_nonblocking(fd)) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint8_t* peer_addr = NULL;
  uint32_t peer_addr_len = 0;
  if (!x07_ext_sockaddr_to_netaddr_v1((struct sockaddr*)&peer, 0, &peer_addr, &peer_addr_len)) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint32_t stream_handle = x07_ext_sockets_alloc_sock_slot(X07_SOCK_KIND_TCP_STREAM, fd);
  if (!stream_handle) {
    free(peer_addr);
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_handle_peer_doc(stream_handle, peer_addr, peer_addr_len, &doc_len);
  free(peer_addr);
  if (!doc) {
    (void)x07_ext_sockets_sock_drop(stream_handle, X07_SOCK_KIND_TCP_STREAM);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    (void)x07_ext_sockets_sock_drop(stream_handle, X07_SOCK_KIND_TCP_STREAM);
    return -1;
  }
  return 0;
}

static int32_t x07_ext_sockets_tls_stream_read_alloc(
  uint32_t stream_handle,
  uint32_t max_bytes,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
);

static int32_t x07_ext_sockets_tls_stream_write_alloc(
  uint32_t stream_handle,
  const uint8_t* data,
  uint32_t data_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
);

static int32_t x07_ext_sockets_tls_stream_wait_alloc(
  uint32_t stream_handle,
  uint32_t events,
  int32_t timeout_ms,
  uint32_t* out_handle
);

int32_t x07_ext_sockets_tcp_stream_read_alloc(
  uint32_t stream_handle,
  uint32_t max_bytes,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (e->kind == X07_SOCK_KIND_TLS_STREAM) {
    return x07_ext_sockets_tls_stream_read_alloc(stream_handle, max_bytes, caps, caps_len, out_handle);
  }

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (max_bytes > c.max_read_bytes) max_bytes = c.max_read_bytes;

  if (e->kind != X07_SOCK_KIND_TCP_STREAM) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  int revents = 0;
  int prc = x07_ext_poll_one(e->fd, POLLIN, (int)c.io_timeout_ms, &revents);
  if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
  if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  uint8_t* tmp = NULL;
  if (max_bytes != 0) {
    tmp = (uint8_t*)malloc((size_t)max_bytes);
    if (!tmp) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  ssize_t got = recv(e->fd, tmp, (size_t)max_bytes, 0);
  if (got < 0) {
    int err = x07_sock_errno();
    free(tmp);
    if (err == EAGAIN || err == EWOULDBLOCK) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
    return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  }
  uint32_t payload_len = (got > 0) ? (uint32_t)got : 0u;

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_read_doc(tmp, payload_len, &doc_len);
  free(tmp);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tcp_stream_write_alloc(
  uint32_t stream_handle,
  const uint8_t* data,
  uint32_t data_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (e->kind == X07_SOCK_KIND_TLS_STREAM) {
    return x07_ext_sockets_tls_stream_write_alloc(stream_handle, data, data_len, caps, caps_len, out_handle);
  }

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (data_len > c.max_write_bytes) return x07_ext_return_err(X07_NET_ERR_TOO_LARGE, out_handle);

  if (e->kind != X07_SOCK_KIND_TCP_STREAM) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  int revents = 0;
  int prc = x07_ext_poll_one(e->fd, POLLOUT, (int)c.io_timeout_ms, &revents);
  if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
  if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  uint32_t wrote = 0;
  if (data_len != 0) {
    ssize_t n = send(e->fd, data, (size_t)data_len, 0);
    if (n < 0) {
      int err = x07_sock_errno();
      if (err == EAGAIN || err == EWOULDBLOCK) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
      return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
    }
    if (n > 0) wrote = (uint32_t)n;
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_write_doc(wrote, &doc_len);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tcp_stream_wait_alloc(
  uint32_t stream_handle,
  uint32_t events,
  int32_t timeout_ms,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (timeout_ms < 0) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (e->kind == X07_SOCK_KIND_TLS_STREAM) {
    return x07_ext_sockets_tls_stream_wait_alloc(stream_handle, events, timeout_ms, out_handle);
  }
  if (e->kind != X07_SOCK_KIND_TCP_STREAM) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  int pevents = 0;
  if ((events & 1u) != 0u) pevents |= POLLIN;
  if ((events & 2u) != 0u) pevents |= POLLOUT;
  // Hangup is reported via revents.

  int revents = 0;
  int prc = x07_ext_poll_one(e->fd, pevents, timeout_ms, &revents);
  if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  uint32_t out_events = 0;
  if (prc == 0) {
    out_events = 0;
  } else {
    if ((revents & POLLIN) != 0) out_events |= 1u;
    if ((revents & POLLOUT) != 0) out_events |= 2u;
    if ((revents & (POLLHUP | POLLERR | POLLNVAL)) != 0) out_events |= 4u;
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_wait_doc(out_events, &doc_len);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

static int32_t x07_ext_sockets_tls_stream_read_alloc(
  uint32_t stream_handle,
  uint32_t max_bytes,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (max_bytes > c.max_read_bytes) max_bytes = c.max_read_bytes;

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->kind != X07_SOCK_KIND_TLS_STREAM || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  SSL* ssl = g_tls[stream_handle].ssl;
  if (!ssl) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  if (max_bytes == 0) {
    uint32_t doc_len = 0;
    uint8_t* doc = x07_ext_make_ok_read_doc(NULL, 0, &doc_len);
    if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
    if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
      free(doc);
      return -1;
    }
    return 0;
  }

  uint8_t* tmp = (uint8_t*)malloc((size_t)max_bytes);
  if (!tmp) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  for (;;) {
    ERR_clear_error();
    int got = SSL_read(ssl, (char*)tmp, (int)max_bytes);
    if (got > 0) {
      uint32_t payload_len = (uint32_t)got;
      uint32_t doc_len = 0;
      uint8_t* doc = x07_ext_make_ok_read_doc(tmp, payload_len, &doc_len);
      free(tmp);
      if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
      if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
        free(doc);
        return -1;
      }
      return 0;
    }

    int err = SSL_get_error(ssl, got);
    if (err == SSL_ERROR_ZERO_RETURN) {
      uint32_t doc_len = 0;
      uint8_t* doc = x07_ext_make_ok_read_doc(NULL, 0, &doc_len);
      free(tmp);
      if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
      if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
        free(doc);
        return -1;
      }
      return 0;
    }
    if (err == SSL_ERROR_WANT_READ) {
      int revents = 0;
      int prc = x07_ext_poll_one(e->fd, POLLIN, (int)c.io_timeout_ms, &revents);
      if (prc == 0) {
        free(tmp);
        return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
      }
      if (prc < 0) {
        free(tmp);
        return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
      }
      continue;
    }
    if (err == SSL_ERROR_WANT_WRITE) {
      int revents = 0;
      int prc = x07_ext_poll_one(e->fd, POLLOUT, (int)c.io_timeout_ms, &revents);
      if (prc == 0) {
        free(tmp);
        return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
      }
      if (prc < 0) {
        free(tmp);
        return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
      }
      continue;
    }

    free(tmp);
    return x07_ext_return_err(X07_NET_ERR_TLS, out_handle);
  }
}

static int32_t x07_ext_sockets_tls_stream_write_alloc(
  uint32_t stream_handle,
  const uint8_t* data,
  uint32_t data_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (data_len > c.max_write_bytes) return x07_ext_return_err(X07_NET_ERR_TOO_LARGE, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->kind != X07_SOCK_KIND_TLS_STREAM || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  SSL* ssl = g_tls[stream_handle].ssl;
  if (!ssl) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  uint32_t wrote = 0;
  if (data_len != 0) {
    for (;;) {
      ERR_clear_error();
      int n = SSL_write(ssl, (const char*)data, (int)data_len);
      if (n > 0) {
        wrote = (uint32_t)n;
        break;
      }

      int err = SSL_get_error(ssl, n);
      if (err == SSL_ERROR_WANT_READ) {
        int revents = 0;
        int prc = x07_ext_poll_one(e->fd, POLLIN, (int)c.io_timeout_ms, &revents);
        if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
        if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
        continue;
      }
      if (err == SSL_ERROR_WANT_WRITE) {
        int revents = 0;
        int prc = x07_ext_poll_one(e->fd, POLLOUT, (int)c.io_timeout_ms, &revents);
        if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
        if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
        continue;
      }
      return x07_ext_return_err(X07_NET_ERR_TLS, out_handle);
    }
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_write_doc(wrote, &doc_len);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

static int32_t x07_ext_sockets_tls_stream_wait_alloc(
  uint32_t stream_handle,
  uint32_t events,
  int32_t timeout_ms,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (timeout_ms < 0) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(stream_handle);
  if (!e || e->kind != X07_SOCK_KIND_TLS_STREAM || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  SSL* ssl = g_tls[stream_handle].ssl;
  if (!ssl) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  uint32_t out_events = 0;
  if ((events & 1u) != 0u) {
    if (SSL_pending(ssl) > 0) out_events |= 1u;
  }

  if (out_events == 0) {
    int pevents = 0;
    if ((events & 1u) != 0u) pevents |= POLLIN;
    if ((events & 2u) != 0u) pevents |= POLLOUT;

    int revents = 0;
    int prc = x07_ext_poll_one(e->fd, pevents, timeout_ms, &revents);
    if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

    if (prc != 0) {
      if ((revents & POLLIN) != 0) out_events |= 1u;
      if ((revents & POLLOUT) != 0) out_events |= 2u;
      if ((revents & (POLLHUP | POLLERR | POLLNVAL)) != 0) out_events |= 4u;
    }
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_wait_doc(out_events, &doc_len);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_tcp_stream_shutdown_v1(int32_t stream_handle, int32_t how) {
  if (!x07_ext_wsa_ready()) return 0;
  if (stream_handle <= 0) return 0;
  x07SockEntry* e = x07_ext_sockets_sock_ptr((uint32_t)stream_handle);
  if (!e || e->kind != X07_SOCK_KIND_TCP_STREAM || e->closed) return 0;

  int sh = SHUT_RDWR;
  if (how == 0) sh = SHUT_RD;
  else if (how == 1) sh = SHUT_WR;
  else if (how == 2) sh = SHUT_RDWR;
  else return 0;
  return shutdown(e->fd, sh) == 0 ? 1 : 0;
}

int32_t x07_ext_sockets_tcp_stream_close_v1(int32_t stream_handle) {
  if (stream_handle <= 0) return 0;
  x07SockEntry* e = x07_ext_sockets_sock_ptr((uint32_t)stream_handle);
  if (!e || e->closed) return 0;
  if (e->kind != X07_SOCK_KIND_TCP_STREAM && e->kind != X07_SOCK_KIND_TLS_STREAM) return 0;
  x07_ext_sockets_sock_close_in_place(e);
  return 1;
}

int32_t x07_ext_sockets_tcp_stream_drop_v1(int32_t stream_handle) {
  if (stream_handle <= 0) return 0;
  x07SockEntry* e = x07_ext_sockets_sock_ptr((uint32_t)stream_handle);
  if (!e) return 0;
  if (e->kind != X07_SOCK_KIND_TCP_STREAM && e->kind != X07_SOCK_KIND_TLS_STREAM) return 0;
  x07_ext_sockets_sock_close_in_place(e);
  memset(e, 0, sizeof(*e));
  return 1;
}

int32_t x07_ext_sockets_tcp_listener_close_v1(int32_t listener_handle) {
  if (listener_handle <= 0) return 0;
  return x07_ext_sockets_sock_close((uint32_t)listener_handle, X07_SOCK_KIND_TCP_LISTENER);
}

int32_t x07_ext_sockets_tcp_listener_drop_v1(int32_t listener_handle) {
  if (listener_handle <= 0) return 0;
  return x07_ext_sockets_sock_drop((uint32_t)listener_handle, X07_SOCK_KIND_TCP_LISTENER);
}

int32_t x07_ext_sockets_udp_bind_alloc(
  const uint8_t* addr,
  uint32_t addr_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07NetAddrV1 a;
  if (!x07_ext_parse_addr_v1(addr, addr_len, &a)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (a.tag != X07_NET_ADDR_TAG_IPV4 && a.tag != X07_NET_ADDR_TAG_IPV6) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  if (x07_ext_env_is_1("X07_OS_SANDBOXED")) {
    if (!x07_ext_env_is_1("X07_OS_NET")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_UDP")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_addr_policy_check_sandboxed(&a, a.port)) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
  }

  struct sockaddr_storage ss;
  x07_socklen_t ss_len = 0;
  memset(&ss, 0, sizeof(ss));

  if (a.tag == X07_NET_ADDR_TAG_IPV4) {
    struct sockaddr_in in;
    memset(&in, 0, sizeof(in));
    in.sin_family = AF_INET;
    in.sin_port = htons((uint16_t)a.port);
    memcpy(&in.sin_addr, a.ip, 4);
    memcpy(&ss, &in, sizeof(in));
    ss_len = (x07_socklen_t)sizeof(in);
  } else {
    struct sockaddr_in6 in6;
    memset(&in6, 0, sizeof(in6));
    in6.sin6_family = AF_INET6;
    in6.sin6_port = htons((uint16_t)a.port);
    memcpy(&in6.sin6_addr, a.ip, 16);
    memcpy(&ss, &in6, sizeof(in6));
    ss_len = (x07_socklen_t)sizeof(in6);
  }

  x07_sock_t fd = socket(ss.ss_family, SOCK_DGRAM, IPPROTO_UDP);
  if (fd == X07_SOCK_INVALID) return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  if (!x07_ext_sock_set_nonblocking(fd)) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  if (bind(fd, (struct sockaddr*)&ss, (x07_socklen_t)ss_len) != 0) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  }

  struct sockaddr_storage bound;
  x07_socklen_t bound_len = (x07_socklen_t)sizeof(bound);
  memset(&bound, 0, sizeof(bound));
  if (getsockname(fd, (struct sockaddr*)&bound, &bound_len) != 0) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint8_t* bound_addr = NULL;
  uint32_t bound_addr_len = 0;
  if (!x07_ext_sockaddr_to_netaddr_v1((struct sockaddr*)&bound, 0, &bound_addr, &bound_addr_len)) {
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  if (x07_ext_env_is_1("X07_OS_SANDBOXED")) {
    x07NetAddrV1 ba;
    if (!x07_ext_parse_addr_v1(bound_addr, bound_addr_len, &ba) || !x07_ext_addr_policy_check_sandboxed(&ba, ba.port)) {
      free(bound_addr);
      (void)x07_sock_close(fd);
      return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    }
  }

  uint32_t sock_handle = x07_ext_sockets_alloc_sock_slot(X07_SOCK_KIND_UDP, fd);
  if (!sock_handle) {
    free(bound_addr);
    (void)x07_sock_close(fd);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_handle_addr_doc(sock_handle, bound_addr, bound_addr_len, &doc_len);
  free(bound_addr);
  if (!doc) {
    (void)x07_ext_sockets_sock_drop(sock_handle, X07_SOCK_KIND_UDP);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    (void)x07_ext_sockets_sock_drop(sock_handle, X07_SOCK_KIND_UDP);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_udp_sendto_alloc(
  uint32_t sock_handle,
  const uint8_t* addr,
  uint32_t addr_len,
  const uint8_t* payload,
  uint32_t payload_len,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (payload_len > c.max_write_bytes) return x07_ext_return_err(X07_NET_ERR_TOO_LARGE, out_handle);

  x07SockEntry* e = x07_ext_sockets_sock_ptr(sock_handle);
  if (!e || e->kind != X07_SOCK_KIND_UDP || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  x07NetAddrV1 a;
  if (!x07_ext_parse_addr_v1(addr, addr_len, &a)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  if (x07_ext_env_is_1("X07_OS_SANDBOXED")) {
    if (!x07_ext_env_is_1("X07_OS_NET")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_env_is_1("X07_OS_NET_ALLOW_UDP")) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (!x07_ext_addr_policy_check_sandboxed(&a, a.port)) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
  }

  struct sockaddr_storage ss;
  x07_socklen_t ss_len = 0;
  memset(&ss, 0, sizeof(ss));

  if (a.tag == X07_NET_ADDR_TAG_DNS) {
    char host[1025];
    if (a.dns_len > 1024u) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
    memcpy(host, a.dns, (size_t)a.dns_len);
    host[a.dns_len] = '\0';

    char service[16];
    snprintf(service, sizeof(service), "%u", (unsigned)a.port);

    struct addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_DGRAM;
    hints.ai_protocol = IPPROTO_UDP;
    struct addrinfo* res = NULL;
    int gai = getaddrinfo(host, service, &hints, &res);
    if (gai != 0 || !res) return x07_ext_return_err(X07_NET_ERR_DNS, out_handle);

    const char* allow_hosts = NULL;
    int sandboxed = x07_ext_env_is_1("X07_OS_SANDBOXED");
    if (sandboxed) {
      allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
      if (!allow_hosts || !*allow_hosts) {
        freeaddrinfo(res);
        return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
      }
    }

    int revents = 0;
    int prc = x07_ext_poll_one(e->fd, POLLOUT, (int)c.io_timeout_ms, &revents);
    if (prc == 0) {
      freeaddrinfo(res);
      return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
    }
    if (prc < 0) {
      freeaddrinfo(res);
      return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
    }

    int ok_addr = 0;
    int saw_timeout = 0;
    int saw_connect_err = 0;
    for (struct addrinfo* it = res; it; it = it->ai_next) {
      if (!it->ai_addr || it->ai_addrlen == 0 || it->ai_addrlen > (x07_socklen_t)sizeof(ss)) continue;

      if (sandboxed) {
        char host_buf[INET6_ADDRSTRLEN];
        memset(host_buf, 0, sizeof(host_buf));
        if (it->ai_addr->sa_family == AF_INET) {
          const struct sockaddr_in* in = (const struct sockaddr_in*)it->ai_addr;
          const char* s = x07_ext_inet_ntop(AF_INET, &in->sin_addr, host_buf, sizeof(host_buf));
          if (!s) continue;
        } else if (it->ai_addr->sa_family == AF_INET6) {
          const struct sockaddr_in6* in6 = (const struct sockaddr_in6*)it->ai_addr;
          const char* s = x07_ext_inet_ntop(AF_INET6, &in6->sin6_addr, host_buf, sizeof(host_buf));
          if (!s) continue;
        } else {
          continue;
        }
        if (!x07_ext_host_port_allowed(allow_hosts, (const uint8_t*)host_buf, strlen(host_buf), a.port)) continue;
      }

      ok_addr = 1;
      memcpy(&ss, it->ai_addr, it->ai_addrlen);
      ss_len = (x07_socklen_t)it->ai_addrlen;

      ssize_t sent = sendto(e->fd, payload, (size_t)payload_len, 0, (struct sockaddr*)&ss, (x07_socklen_t)ss_len);
      if (sent < 0) {
        int err = x07_sock_errno();
        if (err == EAGAIN || err == EWOULDBLOCK) {
          saw_timeout = 1;
          break;
        }
        saw_connect_err = 1;
        continue;
      }
      uint32_t wrote = (sent > 0) ? (uint32_t)sent : 0u;

      freeaddrinfo(res);
      uint32_t doc_len = 0;
      uint8_t* doc = x07_ext_make_ok_write_doc(wrote, &doc_len);
      if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
      if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
        free(doc);
        return -1;
      }
      return 0;
    }
    freeaddrinfo(res);
    if (sandboxed && !ok_addr) return x07_ext_return_err(X07_NET_ERR_POLICY_DENIED, out_handle);
    if (saw_timeout) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
    if (saw_connect_err) return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
    return x07_ext_return_err(X07_NET_ERR_DNS, out_handle);
  } else if (a.tag == X07_NET_ADDR_TAG_IPV4) {
    struct sockaddr_in in;
    memset(&in, 0, sizeof(in));
    in.sin_family = AF_INET;
    in.sin_port = htons((uint16_t)a.port);
    memcpy(&in.sin_addr, a.ip, 4);
    memcpy(&ss, &in, sizeof(in));
    ss_len = (x07_socklen_t)sizeof(in);
  } else if (a.tag == X07_NET_ADDR_TAG_IPV6) {
    struct sockaddr_in6 in6;
    memset(&in6, 0, sizeof(in6));
    in6.sin6_family = AF_INET6;
    in6.sin6_port = htons((uint16_t)a.port);
    memcpy(&in6.sin6_addr, a.ip, 16);
    memcpy(&ss, &in6, sizeof(in6));
    ss_len = (x07_socklen_t)sizeof(in6);
  } else {
    return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  }

  int revents = 0;
  int prc = x07_ext_poll_one(e->fd, POLLOUT, (int)c.io_timeout_ms, &revents);
  if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
  if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  ssize_t sent = sendto(e->fd, payload, (size_t)payload_len, 0, (struct sockaddr*)&ss, (x07_socklen_t)ss_len);
  if (sent < 0) {
    int err = x07_sock_errno();
    if (err == EAGAIN || err == EWOULDBLOCK) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
    return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  }
  uint32_t wrote = (sent > 0) ? (uint32_t)sent : 0u;

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_write_doc(wrote, &doc_len);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_udp_recvfrom_alloc(
  uint32_t sock_handle,
  uint32_t max_bytes,
  const uint8_t* caps,
  uint32_t caps_len,
  uint32_t* out_handle
) {
  if (out_handle) *out_handle = 0;
  if (!x07_ext_wsa_ready()) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  x07NetCapsV1 c;
  if (!x07_ext_parse_caps_v1(caps, caps_len, &c)) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);
  if (max_bytes > c.max_read_bytes) max_bytes = c.max_read_bytes;

  x07SockEntry* e = x07_ext_sockets_sock_ptr(sock_handle);
  if (!e || e->kind != X07_SOCK_KIND_UDP || e->closed) return x07_ext_return_err(X07_NET_ERR_INVALID_REQ, out_handle);

  int revents = 0;
  int prc = x07_ext_poll_one(e->fd, POLLIN, (int)c.io_timeout_ms, &revents);
  if (prc == 0) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
  if (prc < 0) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);

  uint8_t* tmp = NULL;
  if (max_bytes != 0) {
    tmp = (uint8_t*)malloc((size_t)max_bytes);
    if (!tmp) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  struct sockaddr_storage peer;
  x07_socklen_t peer_len = (x07_socklen_t)sizeof(peer);
  memset(&peer, 0, sizeof(peer));

  ssize_t got = recvfrom(e->fd, tmp, (size_t)max_bytes, 0, (struct sockaddr*)&peer, &peer_len);
  if (got < 0) {
    int err = x07_sock_errno();
    free(tmp);
    if (err == EAGAIN || err == EWOULDBLOCK) return x07_ext_return_err(X07_NET_ERR_TIMEOUT, out_handle);
    return x07_ext_return_err(X07_NET_ERR_CONNECT, out_handle);
  }
  uint32_t payload_len = (got > 0) ? (uint32_t)got : 0u;

  uint8_t* from_addr = NULL;
  uint32_t from_addr_len = 0;
  if (!x07_ext_sockaddr_to_netaddr_v1((struct sockaddr*)&peer, 0, &from_addr, &from_addr_len)) {
    free(tmp);
    return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  }

  uint32_t doc_len = 0;
  uint8_t* doc = x07_ext_make_ok_udp_recv_doc(from_addr, from_addr_len, tmp, payload_len, &doc_len);
  free(from_addr);
  free(tmp);
  if (!doc) return x07_ext_return_err(X07_NET_ERR_INTERNAL, out_handle);
  if (!x07_ext_store_doc(doc, doc_len, out_handle)) {
    free(doc);
    return -1;
  }
  return 0;
}

int32_t x07_ext_sockets_udp_close_v1(int32_t sock_handle) {
  if (sock_handle <= 0) return 0;
  return x07_ext_sockets_sock_close((uint32_t)sock_handle, X07_SOCK_KIND_UDP);
}

int32_t x07_ext_sockets_udp_drop_v1(int32_t sock_handle) {
  if (sock_handle <= 0) return 0;
  return x07_ext_sockets_sock_drop((uint32_t)sock_handle, X07_SOCK_KIND_UDP);
}
