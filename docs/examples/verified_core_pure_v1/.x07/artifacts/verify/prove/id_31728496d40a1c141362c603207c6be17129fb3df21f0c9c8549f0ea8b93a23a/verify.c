#define X07_FREESTANDING 1

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif

#ifndef X07_FREESTANDING
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>
#include <poll.h>
#include <spawn.h>
#include <sys/mman.h>
#include <sys/wait.h>
#ifndef MAP_ANON
#define MAP_ANON MAP_ANONYMOUS
#endif
#else
#include <stddef.h>
#include <stdint.h>

void* memcpy(void* dst, const void* src, size_t n);
void* memmove(void* dst, const void* src, size_t n);
void* memset(void* dst, int c, size_t n);
int memcmp(const void* a, const void* b, size_t n);
char* getenv(const char* name);
int snprintf(char* s, size_t n, const char* fmt, ...);
#endif

#ifndef X07_MEM_CAP
#define X07_MEM_CAP (64u * 1024u * 1024u)
#endif

#ifndef X07_FUEL_INIT
#define X07_FUEL_INIT 50000000ULL
#endif

#ifndef X07_ENABLE_FS
#define X07_ENABLE_FS 0
#endif

#ifndef X07_ENABLE_RR
#define X07_ENABLE_RR 0
#endif

#ifndef X07_ENABLE_KV
#define X07_ENABLE_KV 0
#endif

#define X07_ENABLE_STREAMING_FILE_IO (X07_ENABLE_FS || X07_ENABLE_RR || X07_ENABLE_KV)

#ifdef X07_FREESTANDING
#if X07_ENABLE_FS || X07_ENABLE_RR || X07_ENABLE_KV
#error "X07_FREESTANDING requires X07_ENABLE_FS/RR/KV=0"
#endif
#endif

#ifdef X07_DEBUG_BORROW
#ifndef X07_DBG_ALLOC_CAP
#define X07_DBG_ALLOC_CAP 65536u
#endif
#ifndef X07_DBG_BORROW_CAP
#define X07_DBG_BORROW_CAP 65536u
#endif
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} bytes_t;

typedef struct {
  uint8_t* ptr;
  uint32_t len;
#ifdef X07_DEBUG_BORROW
  uint64_t aid;
  uint64_t bid;
  uint32_t off_bytes;
#endif
} bytes_view_t;

typedef struct {
  uint32_t tag;
  uint32_t payload;
} option_i32_t;

typedef struct {
  uint32_t tag;
  bytes_t payload;
} option_bytes_t;

typedef struct {
  uint32_t tag;
  bytes_view_t payload;
} option_bytes_view_t;

typedef struct {
  uint32_t tag;
  union {
    uint32_t ok;
    uint32_t err;
  } payload;
} result_i32_t;

typedef struct {
  uint32_t tag;
  union {
    bytes_t ok;
    uint32_t err;
  } payload;
} result_bytes_t;

typedef struct {
  uint32_t tag;
  union {
    bytes_view_t ok;
    uint32_t err;
  } payload;
} result_bytes_view_t;

typedef struct {
  uint32_t tag;
  union {
    result_bytes_t ok;
    uint32_t err;
  } payload;
} result_result_bytes_t;

#define RT_TASK_OUT_KIND_BYTES UINT32_C(1)
#define RT_TASK_OUT_KIND_RESULT_BYTES UINT32_C(2)

typedef struct {
  uint32_t kind;
  union {
    bytes_t bytes;
    result_bytes_t result_bytes;
  } payload;
} rt_task_out_t;

typedef struct {
  uint32_t data;
  uint32_t vtable;
} iface_t;

static __attribute__((noreturn)) void rt_trap(const char* msg);
static __attribute__((noreturn)) void rt_trap_path(const char* msg, const char* path);

#define RT_IFACE_VTABLE_IO_READER UINT32_C(1)
#define RT_IFACE_VTABLE_EXT_IO_READER_MIN UINT32_C(2)
#define RT_IFACE_VTABLE_EXT_IO_READER_MAX UINT32_C(64)

typedef uint32_t (*rt_ext_io_reader_read_fn_t)(uint32_t data, uint8_t* dst, uint32_t cap);
typedef void (*rt_ext_io_reader_drop_fn_t)(uint32_t data);

typedef struct {
  rt_ext_io_reader_read_fn_t read;
  rt_ext_io_reader_drop_fn_t drop;
} rt_ext_io_reader_vtable_t;

static rt_ext_io_reader_vtable_t rt_ext_io_reader_vtables[
  RT_IFACE_VTABLE_EXT_IO_READER_MAX - RT_IFACE_VTABLE_EXT_IO_READER_MIN + 1
];
static uint32_t rt_ext_io_reader_vtables_len = 0;

// External packages register IO reader vtables at runtime to enable `iface` streaming
// through `io.read` / `bufread.*` without adding new builtins.
uint32_t x07_rt_register_io_reader_vtable_v1(
  rt_ext_io_reader_read_fn_t read,
  rt_ext_io_reader_drop_fn_t drop
) {
  if (!read) return 0;

  for (uint32_t i = 0; i < rt_ext_io_reader_vtables_len; i++) {
    rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[i];
    if (vt->read == read && vt->drop == drop) {
      return RT_IFACE_VTABLE_EXT_IO_READER_MIN + i;
    }
  }

  uint32_t cap = (uint32_t)(sizeof(rt_ext_io_reader_vtables) / sizeof(rt_ext_io_reader_vtables[0]));
  if (rt_ext_io_reader_vtables_len >= cap) return 0;

  rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[rt_ext_io_reader_vtables_len];
  vt->read = read;
  vt->drop = drop;

  uint32_t id = RT_IFACE_VTABLE_EXT_IO_READER_MIN + rt_ext_io_reader_vtables_len;
  rt_ext_io_reader_vtables_len += 1;
  return id;
}

static uint32_t rt_ext_io_reader_read_into(uint32_t vtable, uint32_t data, uint8_t* dst, uint32_t cap) {
  if (vtable < RT_IFACE_VTABLE_EXT_IO_READER_MIN || vtable > RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    rt_trap("ext io reader invalid vtable");
  }
  uint32_t idx = vtable - RT_IFACE_VTABLE_EXT_IO_READER_MIN;
  if (idx >= rt_ext_io_reader_vtables_len) {
    rt_trap("ext io reader unregistered vtable");
  }
  rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[idx];
  if (!vt->read) {
    rt_trap("ext io reader missing read fn");
  }
  uint32_t got = vt->read(data, dst, cap);
  if (got > cap) {
    rt_trap("ext io reader returned too many bytes");
  }
  return got;
}

typedef struct {
  bytes_t key;
  bytes_t val;
} kv_entry_t;

typedef struct {
  bytes_t path;
  uint32_t ticks;
} fs_latency_entry_t;

typedef struct {
  uint32_t used;

  uint32_t latency_ticks;

  uint32_t kind_off;
  uint32_t kind_len;

  uint32_t op_off;
  uint32_t op_len;

  uint32_t key_off;
  uint32_t key_len;

  uint32_t payload_off;
  uint32_t payload_len;
} rr_entry_desc_t;

typedef struct {
  bytes_t path;
  bytes_t blob;
  rr_entry_desc_t* entries;
  uint32_t entries_len;
  uint32_t entries_cap;

  uint64_t file_bytes;
  void* append_f;
} rr_cassette_t;

typedef struct {
  uint32_t alive;
  uint8_t mode;
  uint8_t match_mode;
  uint16_t reserved;

  uint64_t max_cassette_bytes;
  uint32_t max_entries;
  uint32_t max_req_bytes;
  uint32_t max_resp_bytes;
  uint32_t max_key_bytes;

  uint32_t transcript_cassette;
  uint32_t transcript_idx;

  rr_cassette_t* cassettes;
  uint32_t cassettes_len;
  uint32_t cassettes_cap;
} rr_handle_t;

typedef struct {
  bytes_t key;
  uint32_t ticks;
} kv_latency_entry_t;

typedef struct {
  uint8_t* mem;
  uint32_t cap;
  uint32_t free_head;
} heap_t;

// Heap allocator is fixed-capacity and deterministic.
// It uses a singly-linked free list of blocks (sorted by address) with coalescing.
#define RT_HEAP_ALIGN UINT32_C(16)
#define RT_HEAP_MAGIC_FREE UINT32_C(0x45564f46) // "X07F"
#define RT_HEAP_MAGIC_USED UINT32_C(0x45564f55) // "X07U"
#define RT_HEAP_NULL_OFF UINT32_MAX

typedef struct {
  uint32_t size;     // total block size including header, multiple of RT_HEAP_ALIGN
  uint32_t next_off; // free: free-list next (offset from heap base), used: allocation epoch id
  uint32_t magic;    // RT_HEAP_MAGIC_FREE / RT_HEAP_MAGIC_USED
  uint32_t req_size; // requested payload size in bytes
} heap_hdr_t;

static uint32_t rt_heap_align_u32(uint32_t x) {
  return (x + (RT_HEAP_ALIGN - 1u)) & ~(RT_HEAP_ALIGN - 1u);
}

typedef struct {
  uint64_t alloc_calls;
  uint64_t realloc_calls;
  uint64_t free_calls;

  uint64_t bytes_alloc_total;
  uint64_t bytes_freed_total;

  uint64_t live_bytes;
  uint64_t peak_live_bytes;

  uint64_t live_allocs;
  uint64_t peak_live_allocs;

  uint64_t memcpy_bytes;
} mem_stats_t;

typedef struct {
  void* ctx;
  void* (*alloc)(void* ctx, uint32_t size, uint32_t align);
  void* (*realloc)(void* ctx, void* ptr, uint32_t old_size, uint32_t new_size, uint32_t align);
  void (*free)(void* ctx, void* ptr, uint32_t size, uint32_t align);
} allocator_v1_t;

__attribute__((weak)) allocator_v1_t x07_custom_allocator(void) {
  return (allocator_v1_t){0};
}

#ifdef X07_DEBUG_BORROW
typedef struct {
  uint8_t* base_ptr;
  uint32_t size_bytes;
  uint32_t alive;
  uint64_t borrow_id;
} dbg_alloc_rec_t;

typedef struct {
  uint64_t alloc_id;
  uint32_t off_bytes;
  uint32_t len_bytes;
  uint32_t active;
} dbg_borrow_rec_t;
#endif

typedef struct {
  uint64_t tasks_spawned;
  uint64_t spawn_calls;
  uint64_t join_calls;
  uint64_t yield_calls;
  uint64_t sleep_calls;
  uint64_t chan_send_calls;
  uint64_t chan_recv_calls;
  uint64_t ctx_switches;
  uint64_t wake_events;
  uint64_t blocked_waits;
  uint64_t virtual_time_end;
  uint64_t sched_trace_hash;
} sched_stats_t;

typedef struct rt_task_s rt_task_t;
typedef struct rt_timer_ev_s rt_timer_ev_t;
typedef struct rt_chan_bytes_s rt_chan_bytes_t;
typedef struct rt_select_evt_s rt_select_evt_t;
typedef struct rt_io_reader_s rt_io_reader_t;
typedef struct rt_bufread_s rt_bufread_t;
typedef struct rt_scratch_u8_fixed_s rt_scratch_u8_fixed_t;
typedef struct rt_os_proc_s rt_os_proc_t;

#define X07_ASSERT_BYTES_EQ_PREFIX_MAX 64

typedef struct {
  uint64_t fuel_init;
  uint64_t fuel;
  int32_t exit_code;
  uint32_t budget_fuel_depth;
  heap_t heap;
  allocator_v1_t allocator;
  uint32_t allocator_is_custom;

  // Total heap usage counters (all allocations, including input/runtime metadata).
  uint64_t heap_live_bytes;
  uint64_t heap_peak_live_bytes;
  uint64_t heap_live_allocs;
  uint64_t heap_peak_live_allocs;

  // Epoch id for mem_stats tracking (0 means "not started yet").
  uint32_t mem_epoch_id;
  mem_stats_t mem_stats;
#ifdef X07_DEBUG_BORROW
  dbg_alloc_rec_t* dbg_allocs;
  uint32_t dbg_allocs_len;
  uint32_t dbg_allocs_cap;

  dbg_borrow_rec_t* dbg_borrows;
  uint32_t dbg_borrows_len;
  uint32_t dbg_borrows_cap;

  uint64_t dbg_borrow_violations;
#endif
  uint64_t fs_read_file_calls;
  uint64_t fs_list_dir_calls;
  uint64_t rr_open_calls;
  uint64_t rr_close_calls;
  uint64_t rr_stats_calls;
  uint64_t rr_next_calls;
  uint64_t rr_next_miss_calls;
  uint64_t rr_append_calls;

  int32_t rr_current;

  rr_handle_t* rr_handles;
  uint32_t rr_handles_len;
  uint32_t rr_handles_cap;
  uint64_t kv_get_calls;
  uint64_t kv_set_calls;

  // Phase G2 fixture-backed latency indices (loaded lazily).
  uint32_t fs_latency_loaded;
  uint32_t fs_latency_default_ticks;
  fs_latency_entry_t* fs_latency_entries;
  uint32_t fs_latency_len;
  bytes_t fs_latency_blob;

  uint32_t kv_latency_loaded;
  uint32_t kv_latency_default_ticks;
  kv_latency_entry_t* kv_latency_entries;
  uint32_t kv_latency_len;
  bytes_t kv_latency_blob;

  kv_entry_t* kv_items;
  uint32_t kv_len;
  uint32_t kv_cap;

  void** map_u32_items;
  uint32_t map_u32_len;
  uint32_t map_u32_cap;

  void** vec_value_items;
  uint32_t vec_value_len;
  uint32_t vec_value_cap;

  void** map_value_items;
  uint32_t map_value_len;
  uint32_t map_value_cap;

  // Phase G2 scheduler + concurrency (deterministic, single-thread cooperative).
  uint32_t sched_current_task;
  uint64_t sched_now_ticks;
  uint64_t sched_seq;
  sched_stats_t sched_stats;

  rt_task_t* sched_tasks;
  uint32_t sched_tasks_len;
  uint32_t sched_tasks_cap;

  uint32_t sched_ready_head;
  uint32_t sched_ready_tail;

  rt_timer_ev_t* sched_timers;
  uint32_t sched_timers_len;
  uint32_t sched_timers_cap;

  rt_chan_bytes_t* sched_chans;
  uint32_t sched_chans_len;
  uint32_t sched_chans_cap;

  rt_select_evt_t* sched_select_evts;
  uint32_t sched_select_evts_len;
  uint32_t sched_select_evts_cap;

  // Phase G2 streaming I/O (deterministic, fixture-backed).
  rt_io_reader_t* io_readers;
  uint32_t io_readers_len;
  uint32_t io_readers_cap;

  rt_bufread_t* bufreads;
  uint32_t bufreads_len;
  uint32_t bufreads_cap;

  // Deterministic fixed-capacity scratch buffers.
  rt_scratch_u8_fixed_t* scratch_u8_fixed;
  uint32_t scratch_u8_fixed_len;
  uint32_t scratch_u8_fixed_cap;

  // Standalone OS process table (run-os*, non-deterministic).
  rt_os_proc_t* os_procs;
  uint32_t os_procs_len;
  uint32_t os_procs_cap;
  uint32_t os_procs_live;
  uint32_t os_procs_spawned;

  const char* trap_ptr;

  uint32_t last_bytes_eq_valid;
  uint32_t last_bytes_eq_a_len;
  uint32_t last_bytes_eq_b_len;
  uint8_t last_bytes_eq_a_prefix[X07_ASSERT_BYTES_EQ_PREFIX_MAX];
  uint8_t last_bytes_eq_b_prefix[X07_ASSERT_BYTES_EQ_PREFIX_MAX];
} ctx_t;

// Global ctx pointer for native extension backends that need to allocate bytes via the runtime.
static ctx_t* rt_ext_ctx = NULL;

// Native math backend entrypoints (linked from deps/x07/libx07_math.*).
bytes_t ev_math_f64_add_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_sub_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_mul_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_div_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_neg_v1(bytes_t x);
bytes_t ev_math_f64_abs_v1(bytes_t x);
bytes_t ev_math_f64_min_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_max_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_sqrt_v1(bytes_t x);
bytes_t ev_math_f64_sin_v1(bytes_t x);
bytes_t ev_math_f64_cos_v1(bytes_t x);
bytes_t ev_math_f64_tan_v1(bytes_t x);
bytes_t ev_math_f64_exp_v1(bytes_t x);
bytes_t ev_math_f64_ln_v1(bytes_t x);
bytes_t ev_math_f64_pow_v1(bytes_t x, bytes_t y);
bytes_t ev_math_f64_atan2_v1(bytes_t y, bytes_t x);
bytes_t ev_math_f64_floor_v1(bytes_t x);
bytes_t ev_math_f64_ceil_v1(bytes_t x);
bytes_t ev_math_f64_fmt_shortest_v1(bytes_t x);
result_bytes_t ev_math_f64_parse_v1(bytes_t s);
bytes_t ev_math_f64_from_i32_v1(int32_t x);
result_i32_t ev_math_f64_to_i32_trunc_v1(bytes_t x);
bytes_t ev_math_f64_to_bits_u64le_v1(bytes_t x);

// Native time backend entrypoints (linked from deps/x07/libx07_time.*).
uint32_t ev_time_tzdb_is_valid_tzid_v1(bytes_t tzid);
bytes_t ev_time_tzdb_offset_duration_v1(bytes_t tzid, int32_t unix_s_lo, int32_t unix_s_hi);
bytes_t ev_time_tzdb_snapshot_id_v1(void);

// Native ext-fs backend entrypoints (linked from deps/x07/libx07_ext_fs.*).
result_bytes_t x07_ext_fs_read_all_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_write_all_v1(bytes_t path, bytes_t data, bytes_t caps);
result_i32_t x07_ext_fs_append_all_v1(bytes_t path, bytes_t data, bytes_t caps);
result_i32_t x07_ext_fs_mkdirs_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_remove_file_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_remove_dir_all_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_rename_v1(bytes_t src, bytes_t dst, bytes_t caps);
result_bytes_t x07_ext_fs_list_dir_sorted_text_v1(bytes_t path, bytes_t caps);
result_bytes_t x07_ext_fs_walk_glob_sorted_text_v1(bytes_t root, bytes_t glob, bytes_t caps);
result_bytes_t x07_ext_fs_stat_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_stream_open_write_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_stream_write_all_v1(int32_t writer_handle, bytes_t data);
result_i32_t x07_ext_fs_stream_close_v1(int32_t writer_handle);
int32_t x07_ext_fs_stream_drop_v1(int32_t writer_handle);

// Native ext-stdio backend entrypoints (linked from deps/x07/libx07_ext_stdio.*).
result_bytes_t x07_ext_stdio_read_line_v1(bytes_t caps);
result_i32_t x07_ext_stdio_write_stdout_v1(bytes_t data, bytes_t caps);
result_i32_t x07_ext_stdio_write_stderr_v1(bytes_t data, bytes_t caps);
result_i32_t x07_ext_stdio_flush_stdout_v1(void);
result_i32_t x07_ext_stdio_flush_stderr_v1(void);

// Native ext-rand backend entrypoints (linked from deps/x07/libx07_ext_rand.*).
result_bytes_t x07_ext_rand_bytes_v1(int32_t n, bytes_t caps);
result_bytes_t x07_ext_rand_u64_v1(bytes_t caps);

// Native ext-db-sqlite backend entrypoints (linked from deps/x07/libx07_ext_db_sqlite.*).
bytes_t x07_ext_db_sqlite_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-pg backend entrypoints (linked from deps/x07/libx07_ext_db_pg.*).
bytes_t x07_ext_db_pg_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-mysql backend entrypoints (linked from deps/x07/libx07_ext_db_mysql.*).
bytes_t x07_ext_db_mysql_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-redis backend entrypoints (linked from deps/x07/libx07_ext_db_redis.*).
bytes_t x07_ext_db_redis_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_redis_cmd_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_redis_close_v1(bytes_t req, bytes_t caps);

// Native ext-regex backend entrypoints (linked from deps/x07/libx07_ext_regex.*).
bytes_t x07_ext_regex_compile_opts_v1(bytes_t pat, int32_t opts_u32);
bytes_t x07_ext_regex_exec_from_v1(bytes_t compiled, bytes_t text, int32_t start_i32);
bytes_t x07_ext_regex_exec_caps_from_v1(bytes_t compiled, bytes_t text, int32_t start_i32);
bytes_t x07_ext_regex_find_all_x7sl_v1(bytes_t compiled, bytes_t text, int32_t max_matches_i32);
bytes_t x07_ext_regex_split_v1(bytes_t compiled, bytes_t text, int32_t max_parts_i32);
bytes_t x07_ext_regex_replace_all_v1(bytes_t compiled, bytes_t text, bytes_t repl, int32_t cap_limit_i32);

// Native ext-jsonschema backend entrypoints (linked from deps/x07/libx07_ext_jsonschema.*).
bytes_t x07_ext_jsonschema_compile_v1(bytes_t schema_json);
bytes_t x07_ext_jsonschema_validate_v1(bytes_t compiled, bytes_t instance_json);

#ifdef X07_STANDALONE
static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms);
static void rt_os_process_cleanup(ctx_t* ctx);
#else
static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms) {
  (void)ctx;
  (void)poll_timeout_ms;
  return UINT32_C(0);
}
static void rt_os_process_cleanup(ctx_t* ctx) {
  (void)ctx;
}
#endif

static __attribute__((noreturn)) void rt_trap(const char* msg) {

#ifndef X07_FREESTANDING
  if (msg) (void)write(STDERR_FILENO, msg, strlen(msg));
  if (rt_ext_ctx && rt_ext_ctx->trap_ptr) {
    const char* p = rt_ext_ctx->trap_ptr;
    (void)write(STDERR_FILENO, " ptr=", 5);
    (void)write(STDERR_FILENO, p, strlen(p));
  }
  if (msg || (rt_ext_ctx && rt_ext_ctx->trap_ptr)) (void)write(STDERR_FILENO, "\n", 1);
#else
  (void)msg;
#endif
  __builtin_trap();
}

static __attribute__((noreturn)) void rt_trap_path(const char* msg, const char* path) {

#ifndef X07_FREESTANDING
  if (msg) (void)write(STDERR_FILENO, msg, strlen(msg));
  if (path) {
    (void)write(STDERR_FILENO, " path=", 6);
    (void)write(STDERR_FILENO, path, strlen(path));
  }
  if (rt_ext_ctx && rt_ext_ctx->trap_ptr) {
    const char* p = rt_ext_ctx->trap_ptr;
    (void)write(STDERR_FILENO, " ptr=", 5);
    (void)write(STDERR_FILENO, p, strlen(p));
  }
  if (msg || path || (rt_ext_ctx && rt_ext_ctx->trap_ptr)) (void)write(STDERR_FILENO, "\n", 1);
#else
  (void)msg;
  (void)path;
#endif
  __builtin_trap();
}

static void rt_fuel(ctx_t* ctx, uint64_t amount) {
  if (ctx->fuel < amount) {
    if (ctx->budget_fuel_depth != 0) rt_trap("X07T_BUDGET_EXCEEDED_FUEL");
    rt_trap("fuel exhausted");
  }
  ctx->fuel -= amount;
}

static uint32_t rt_align_u32(uint32_t x, uint32_t align) {
  return (x + (align - 1u)) & ~(align - 1u);
}

static uint16_t rt_read_u16_le(const uint8_t* p) {
  return (uint16_t)p[0] | ((uint16_t)p[1] << 8);
}

static uint32_t rt_read_u32_le(const uint8_t* p) {
  return (uint32_t)p[0]
       | ((uint32_t)p[1] << 8)
       | ((uint32_t)p[2] << 16)
       | ((uint32_t)p[3] << 24);
}

static void rt_write_u32_le(uint8_t* p, uint32_t x) {
  p[0] = (uint8_t)(x & UINT32_C(0xFF));
  p[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  p[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  p[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static void rt_heap_init(ctx_t* ctx) {
  if (!ctx->heap.mem) rt_trap("heap mem is NULL");
  uint32_t cap = ctx->heap.cap;
  cap &= ~(RT_HEAP_ALIGN - 1u);
  if (cap < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) rt_trap("heap too small");
  ctx->heap.cap = cap;

  ctx->heap.free_head = 0;
  heap_hdr_t* h = (heap_hdr_t*)(ctx->heap.mem);
  h->size = cap;
  h->next_off = RT_HEAP_NULL_OFF;
  h->magic = RT_HEAP_MAGIC_FREE;
  h->req_size = 0;
}

static heap_hdr_t* rt_heap_hdr_at(ctx_t* ctx, uint32_t off) {
  if (off == RT_HEAP_NULL_OFF) return (heap_hdr_t*)NULL;
  if (off > ctx->heap.cap || ctx->heap.cap - off < (uint32_t)sizeof(heap_hdr_t)) rt_trap("heap corrupt");
  return (heap_hdr_t*)(ctx->heap.mem + off);
}

static uint32_t rt_heap_off_of(ctx_t* ctx, heap_hdr_t* h) {
  uintptr_t base = (uintptr_t)(ctx->heap.mem);
  uintptr_t p = (uintptr_t)h;
  if (p < base) rt_trap("heap ptr underflow");
  uintptr_t off = p - base;
  if (off > (uintptr_t)UINT32_MAX) rt_trap("heap ptr overflow");
  if ((uint32_t)off > ctx->heap.cap) rt_trap("heap ptr oob");
  return (uint32_t)off;
}

static uint32_t rt_heap_is_pow2_u32(uint32_t x) {
  return x != 0 && (x & (x - 1u)) == 0;
}

static void* rt_heap_alloc(ctx_t* ctx, uint32_t size, uint32_t align) {
  if (size == 0) return (void*)ctx->heap.mem;
  if (align == 0) rt_trap("alloc align=0");
  if (!rt_heap_is_pow2_u32(align)) rt_trap("alloc align not pow2");
  if (align > RT_HEAP_ALIGN) rt_trap("alloc align too large");

  uint32_t payload = rt_heap_align_u32(size);
  uint32_t need = (uint32_t)sizeof(heap_hdr_t) + payload;
  need = rt_heap_align_u32(need);
  if (need < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) {
    need = (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN;
  }
  if (need > ctx->heap.cap) return NULL;

  uint32_t prev_off = RT_HEAP_NULL_OFF;
  uint32_t off = ctx->heap.free_head;
  while (off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* h = rt_heap_hdr_at(ctx, off);
    if (h->magic != RT_HEAP_MAGIC_FREE) rt_trap("heap free list corrupt");
    if (h->size >= need) {
      uint32_t next_off = h->next_off;

      // Remove from free list.
      if (prev_off == RT_HEAP_NULL_OFF) {
        ctx->heap.free_head = next_off;
      } else {
        heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
        prev->next_off = next_off;
      }

      uint32_t remaining = h->size - need;
      if (remaining >= (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) {
        uint32_t rem_off = off + need;
        heap_hdr_t* rem = rt_heap_hdr_at(ctx, rem_off);
        rem->size = remaining;
        rem->next_off = next_off;
        rem->magic = RT_HEAP_MAGIC_FREE;
        rem->req_size = 0;

        if (prev_off == RT_HEAP_NULL_OFF) {
          ctx->heap.free_head = rem_off;
        } else {
          heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
          prev->next_off = rem_off;
        }
        h->size = need;
      } else {
        // Don't split; keep whole block.
        need = h->size;
      }

      h->next_off = ctx->mem_epoch_id;
      h->magic = RT_HEAP_MAGIC_USED;
      h->req_size = size;

      void* ptr = (void*)(ctx->heap.mem + off + (uint32_t)sizeof(heap_hdr_t));
      memset(ptr, 0, payload);
      return ptr;
    }
    prev_off = off;
    off = h->next_off;
  }
  return NULL;
}

static void rt_heap_free(ctx_t* ctx, void* ptr) {
  if (!ptr) return;
  if ((uint8_t*)ptr == ctx->heap.mem) return;
  uint8_t* p = (uint8_t*)ptr;
  if (p < ctx->heap.mem + (uint32_t)sizeof(heap_hdr_t)) rt_trap("free oob");
  heap_hdr_t* h = (heap_hdr_t*)(p - (uint32_t)sizeof(heap_hdr_t));
  uint32_t off = rt_heap_off_of(ctx, h);
  if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("double free or corrupt heap");
  uint32_t size = h->size;
  if (size < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) rt_trap("free corrupt size");
  if ((size & (RT_HEAP_ALIGN - 1u)) != 0) rt_trap("free corrupt size");
  if (off > ctx->heap.cap || ctx->heap.cap - off < size) rt_trap("free oob");

  // Insert into free list (sorted by address).
  uint32_t prev_off = RT_HEAP_NULL_OFF;
  uint32_t cur_off = ctx->heap.free_head;
  while (cur_off != RT_HEAP_NULL_OFF && cur_off < off) {
    heap_hdr_t* cur = rt_heap_hdr_at(ctx, cur_off);
    prev_off = cur_off;
    cur_off = cur->next_off;
  }

  h->magic = RT_HEAP_MAGIC_FREE;
  h->next_off = cur_off;
  h->req_size = 0;

  if (prev_off == RT_HEAP_NULL_OFF) {
    ctx->heap.free_head = off;
  } else {
    heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
    prev->next_off = off;
  }

  // Coalesce with next.
  if (cur_off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* next = rt_heap_hdr_at(ctx, cur_off);
    if (off + h->size == cur_off) {
      h->size += next->size;
      h->next_off = next->next_off;
    }
  }

  // Coalesce with prev.
  if (prev_off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
    if (prev_off + prev->size == off) {
      prev->size += h->size;
      prev->next_off = h->next_off;
    }
  }
}

static void rt_mem_epoch_reset(ctx_t* ctx) {
  ctx->mem_epoch_id += 1;
  if (ctx->mem_epoch_id == 0) ctx->mem_epoch_id = 1;

  ctx->mem_stats.alloc_calls = 0;
  ctx->mem_stats.realloc_calls = 0;
  ctx->mem_stats.free_calls = 0;
  ctx->mem_stats.bytes_alloc_total = 0;
  ctx->mem_stats.bytes_freed_total = 0;
  ctx->mem_stats.live_bytes = 0;
  ctx->mem_stats.peak_live_bytes = 0;
  ctx->mem_stats.live_allocs = 0;
  ctx->mem_stats.peak_live_allocs = 0;
  ctx->mem_stats.memcpy_bytes = 0;
}

static void rt_mem_on_alloc(ctx_t* ctx, uint32_t size, uint32_t is_realloc) {
  ctx->heap_live_bytes += (uint64_t)size;
  ctx->heap_live_allocs += 1;
  if (ctx->heap_live_bytes > ctx->heap_peak_live_bytes) {
    ctx->heap_peak_live_bytes = ctx->heap_live_bytes;
  }
  if (ctx->heap_live_allocs > ctx->heap_peak_live_allocs) {
    ctx->heap_peak_live_allocs = ctx->heap_live_allocs;
  }

  if (ctx->mem_epoch_id == 0) return;

  if (is_realloc) {
    ctx->mem_stats.realloc_calls += 1;
  } else {
    ctx->mem_stats.alloc_calls += 1;
  }
  ctx->mem_stats.bytes_alloc_total += (uint64_t)size;
  ctx->mem_stats.live_bytes += (uint64_t)size;
  ctx->mem_stats.live_allocs += 1;
  if (ctx->mem_stats.live_bytes > ctx->mem_stats.peak_live_bytes) {
    ctx->mem_stats.peak_live_bytes = ctx->mem_stats.live_bytes;
  }
  if (ctx->mem_stats.live_allocs > ctx->mem_stats.peak_live_allocs) {
    ctx->mem_stats.peak_live_allocs = ctx->mem_stats.live_allocs;
  }
}

static void rt_mem_on_free(ctx_t* ctx, uint32_t size, uint32_t is_epoch, uint32_t strict) {
  if (ctx->heap_live_bytes < (uint64_t)size) rt_trap("mem free underflow");
  if (ctx->heap_live_allocs == 0) rt_trap("mem free underflow");
  ctx->heap_live_bytes -= (uint64_t)size;
  ctx->heap_live_allocs -= 1;

  if (ctx->mem_epoch_id == 0) return;
  if (!is_epoch) return;

  ctx->mem_stats.free_calls += 1;
  ctx->mem_stats.bytes_freed_total += (uint64_t)size;
  if (ctx->mem_stats.live_bytes < (uint64_t)size || ctx->mem_stats.live_allocs == 0) {
    if (strict) rt_trap("mem epoch underflow");
    return;
  }
  ctx->mem_stats.live_bytes -= (uint64_t)size;
  ctx->mem_stats.live_allocs -= 1;
}

static void rt_mem_on_memcpy(ctx_t* ctx, uint32_t size) {
  if (ctx->mem_epoch_id == 0) return;
  ctx->mem_stats.memcpy_bytes += (uint64_t)size;
}

static uint32_t rt_mem_epoch_pause(ctx_t* ctx) {
  uint32_t saved = ctx->mem_epoch_id;
  ctx->mem_epoch_id = 0;
  return saved;
}

static void rt_mem_epoch_resume(ctx_t* ctx, uint32_t saved) {
  ctx->mem_epoch_id = saved;
}

static void* rt_alloc_raw(ctx_t* ctx, uint32_t size, uint32_t align) {
  void* ptr = rt_heap_alloc(ctx, size, align);
  if (!ptr && size) rt_trap("out of memory");
  return ptr;
}

static void* rt_default_alloc(void* alloc_ctx, uint32_t size, uint32_t align) {
  return rt_alloc_raw((ctx_t*)alloc_ctx, size, align);
}

static void* rt_default_realloc(
    void* alloc_ctx,
    void* ptr,
    uint32_t old_size,
    uint32_t new_size,
    uint32_t align
) {
  (void)ptr;
  (void)old_size;
  return rt_alloc_raw((ctx_t*)alloc_ctx, new_size, align);
}

static void rt_default_free(void* alloc_ctx, void* ptr, uint32_t size, uint32_t align) {
  (void)size;
  (void)align;
  rt_heap_free((ctx_t*)alloc_ctx, ptr);
}

static void rt_allocator_init(ctx_t* ctx) {
  ctx->allocator.ctx = ctx;
  ctx->allocator.alloc = rt_default_alloc;
  ctx->allocator.realloc = rt_default_realloc;
  ctx->allocator.free = rt_default_free;
  ctx->allocator_is_custom = 0;

  allocator_v1_t custom = x07_custom_allocator();
  if (custom.alloc || custom.realloc || custom.free) {
    if (!custom.alloc || !custom.realloc || !custom.free) rt_trap("custom allocator missing hooks");
    ctx->allocator = custom;
    ctx->allocator_is_custom = 1;
  }
}

static void* rt_alloc(ctx_t* ctx, uint32_t size, uint32_t align) {
  if (size == 0) return (void*)ctx->heap.mem;
  if (!ctx->allocator.alloc) rt_trap("allocator.alloc missing");
  void* ptr = ctx->allocator.alloc(ctx->allocator.ctx, size, align);
  if (!ptr && size) rt_trap("allocator.alloc failed");
  rt_mem_on_alloc(ctx, size, 0);
  return ptr;
}

static void* rt_alloc_realloc(
    ctx_t* ctx,
    void* old_ptr,
    uint32_t old_size,
    uint32_t new_size,
    uint32_t align
) {
  if (new_size == 0) return (void*)ctx->heap.mem;
  if (!ctx->allocator.realloc) rt_trap("allocator.realloc missing");
  void* new_ptr =
      ctx->allocator.realloc(ctx->allocator.ctx, old_ptr, old_size, new_size, align);
  if (!new_ptr && new_size) rt_trap("allocator.realloc failed");
  rt_mem_on_alloc(ctx, new_size, old_size != 0);
  return new_ptr;
}

static void rt_free(ctx_t* ctx, void* ptr, uint32_t size, uint32_t align) {
  if (!ptr) return;
  if (size == 0) return;
  uint32_t is_epoch = 1;
  uint32_t strict = ctx->allocator_is_custom ? 0 : 1;
  if (!ctx->allocator_is_custom && (uint8_t*)ptr != ctx->heap.mem) {
    uint8_t* p = (uint8_t*)ptr;
    if (p < ctx->heap.mem + (uint32_t)sizeof(heap_hdr_t)) rt_trap("free oob");
    heap_hdr_t* h = (heap_hdr_t*)(p - (uint32_t)sizeof(heap_hdr_t));
    if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("free corrupt heap");
    is_epoch = (h->next_off == ctx->mem_epoch_id);
  }
  if (!ctx->allocator.free) rt_trap("allocator.free missing");
  ctx->allocator.free(ctx->allocator.ctx, ptr, size, align);
  rt_mem_on_free(ctx, size, is_epoch, strict);
}

static void rt_mem_free_all(ctx_t* ctx) {
  // Bulk reset used at process end.
  ctx->mem_stats.free_calls += 1;
  ctx->mem_stats.bytes_freed_total += ctx->mem_stats.live_bytes;
  ctx->mem_stats.live_bytes = 0;
  ctx->mem_stats.live_allocs = 0;

  ctx->heap_live_bytes = 0;
  ctx->heap_live_allocs = 0;
  rt_heap_init(ctx);
}

#ifdef X07_DEBUG_BORROW
static void rt_dbg_init(ctx_t* ctx) {
  ctx->dbg_allocs_len = 0;
  ctx->dbg_allocs_cap = X07_DBG_ALLOC_CAP;
  ctx->dbg_allocs =
      (dbg_alloc_rec_t*)rt_alloc(
          ctx,
          ctx->dbg_allocs_cap * (uint32_t)sizeof(dbg_alloc_rec_t),
          (uint32_t)_Alignof(dbg_alloc_rec_t)
      );

  ctx->dbg_borrows_len = 0;
  ctx->dbg_borrows_cap = X07_DBG_BORROW_CAP;
  ctx->dbg_borrows =
      (dbg_borrow_rec_t*)rt_alloc(
          ctx,
          ctx->dbg_borrows_cap * (uint32_t)sizeof(dbg_borrow_rec_t),
          (uint32_t)_Alignof(dbg_borrow_rec_t)
      );

  ctx->dbg_borrow_violations = 0;
}

static uint64_t rt_dbg_borrow_acquire(
    ctx_t* ctx,
    uint64_t alloc_id,
    uint32_t off_bytes,
    uint32_t len_bytes
);

static uint64_t rt_dbg_alloc_register(ctx_t* ctx, uint8_t* base_ptr, uint32_t size_bytes) {
  if (size_bytes == 0) return 0;
  if (!ctx->dbg_allocs || ctx->dbg_allocs_cap == 0) return 0;
  if (ctx->dbg_allocs_len >= ctx->dbg_allocs_cap) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t idx = ctx->dbg_allocs_len++;
  dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
  rec->base_ptr = base_ptr;
  rec->size_bytes = size_bytes;
  rec->alive = 1;
  rec->borrow_id = rt_dbg_borrow_acquire(ctx, (uint64_t)idx + 1, 0, size_bytes);
  return (uint64_t)idx + 1;
}

static void rt_dbg_alloc_kill(ctx_t* ctx, uint64_t alloc_id) {
  if (alloc_id == 0) return;
  uint64_t idx = alloc_id - 1;
  if (!ctx->dbg_allocs || idx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return;
  }
  ctx->dbg_allocs[idx].alive = 0;
}

static uint64_t rt_dbg_alloc_find(
    ctx_t* ctx,
    uint8_t* ptr,
    uint32_t len_bytes,
    uint32_t* out_off_bytes
) {
  if (out_off_bytes) *out_off_bytes = 0;
  if (len_bytes == 0) return 0;
  if (!ctx->dbg_allocs) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uintptr_t p = (uintptr_t)ptr;
  // Search newest-to-oldest so re-used pointers resolve to the latest allocation record.
  for (uint32_t i = ctx->dbg_allocs_len; i > 0; i--) {
    uint32_t idx = i - 1;
    dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
    uintptr_t base = (uintptr_t)rec->base_ptr;
    uintptr_t end = base + (uintptr_t)rec->size_bytes;
    if (end < base) continue;
    if (p >= base && p < end) {
      uint32_t off = (uint32_t)(p - base);
      if (off > rec->size_bytes || rec->size_bytes - off < len_bytes) {
        ctx->dbg_borrow_violations += 1;
        return 0;
      }
      if (out_off_bytes) *out_off_bytes = off;
      return (uint64_t)idx + 1;
    }
  }
  ctx->dbg_borrow_violations += 1;
  return 0;
}

static uint64_t rt_dbg_alloc_try_find(
    ctx_t* ctx,
    uint8_t* ptr,
    uint32_t len_bytes,
    uint32_t* out_off_bytes
) {
  if (out_off_bytes) *out_off_bytes = 0;
  if (len_bytes == 0) return 0;
  if (!ctx->dbg_allocs) return 0;
  uintptr_t p = (uintptr_t)ptr;
  // Search newest-to-oldest so re-used pointers resolve to the latest allocation record.
  for (uint32_t i = ctx->dbg_allocs_len; i > 0; i--) {
    uint32_t idx = i - 1;
    dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
    uintptr_t base = (uintptr_t)rec->base_ptr;
    uintptr_t end = base + (uintptr_t)rec->size_bytes;
    if (end < base) continue;
    if (p >= base && p < end) {
      uint32_t off = (uint32_t)(p - base);
      if (off > rec->size_bytes || rec->size_bytes - off < len_bytes) return 0;
      if (out_off_bytes) *out_off_bytes = off;
      return (uint64_t)idx + 1;
    }
  }
  return 0;
}

static uint64_t rt_dbg_alloc_borrow_id(ctx_t* ctx, uint64_t alloc_id) {
  if (alloc_id == 0) return 0;
  uint64_t idx = alloc_id - 1;
  if (!ctx->dbg_allocs || idx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t bid = ctx->dbg_allocs[idx].borrow_id;
  if (bid == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return bid;
}

static uint64_t rt_dbg_borrow_acquire(
    ctx_t* ctx,
    uint64_t alloc_id,
    uint32_t off_bytes,
    uint32_t len_bytes
) {
  if (len_bytes == 0) return 0;
  if (alloc_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t aidx = alloc_id - 1;
  if (!ctx->dbg_allocs || aidx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aidx];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes > a->size_bytes || a->size_bytes - off_bytes < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (!ctx->dbg_borrows || ctx->dbg_borrows_cap == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (ctx->dbg_borrows_len >= ctx->dbg_borrows_cap) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t idx = ctx->dbg_borrows_len++;
  dbg_borrow_rec_t* b = &ctx->dbg_borrows[idx];
  b->alloc_id = alloc_id;
  b->off_bytes = off_bytes;
  b->len_bytes = len_bytes;
  b->active = 1;
  return (uint64_t)idx + 1;
}

static void rt_dbg_borrow_release(ctx_t* ctx, uint64_t borrow_id) {
  if (borrow_id == 0) return;
  uint64_t idx = borrow_id - 1;
  if (!ctx->dbg_borrows || idx >= (uint64_t)ctx->dbg_borrows_len) {
    ctx->dbg_borrow_violations += 1;
    return;
  }
  ctx->dbg_borrows[idx].active = 0;
}

static uint32_t rt_dbg_borrow_check(
    ctx_t* ctx,
    uint64_t borrow_id,
    uint32_t off_bytes,
    uint32_t len_bytes
) {
  if (len_bytes == 0) return 1;
  if (borrow_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t idx = borrow_id - 1;
  if (!ctx->dbg_borrows || idx >= (uint64_t)ctx->dbg_borrows_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_borrow_rec_t* b = &ctx->dbg_borrows[idx];
  if (!b->active) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t alloc_id = b->alloc_id;
  if (alloc_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t aidx = alloc_id - 1;
  if (!ctx->dbg_allocs || aidx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aidx];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes < b->off_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t rel = off_bytes - b->off_bytes;
  if (rel > b->len_bytes || b->len_bytes - rel < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes > a->size_bytes || a->size_bytes - off_bytes < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return 1;
}
#endif

static inline bytes_t rt_bytes_empty(ctx_t* ctx) {
  bytes_t out;
  out.ptr = ctx->heap.mem;
  out.len = UINT32_C(0);
  return out;
}

static inline bytes_view_t rt_view_empty(ctx_t* ctx) {
  bytes_view_t out;
  out.ptr = ctx->heap.mem;
  out.len = UINT32_C(0);
#ifdef X07_DEBUG_BORROW
  out.aid = 0;
  out.bid = 0;
  out.off_bytes = 0;
#endif
  return out;
}

static inline rt_task_out_t rt_task_out_empty(ctx_t* ctx) {
  rt_task_out_t out;
  out.kind = RT_TASK_OUT_KIND_BYTES;
  out.payload.bytes = rt_bytes_empty(ctx);
  return out;
}

static bytes_t rt_bytes_alloc(ctx_t* ctx, uint32_t len) {
  if (len == 0) return rt_bytes_empty(ctx);
  bytes_t out;
  out.len = len;
  out.ptr = (uint8_t*)rt_alloc(ctx, len, 1);
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_alloc_register(ctx, out.ptr, len);
#endif
  return out;
}

// Native extension hook: allocate bytes using the currently-running ctx allocator.
bytes_t ev_bytes_alloc(uint32_t len) {
  if (!rt_ext_ctx) rt_trap(NULL);
  return rt_bytes_alloc(rt_ext_ctx, len);
}

// Native extension hook: trap without returning.
__attribute__((noreturn)) void ev_trap(int32_t code) {
  (void)code;
  rt_trap(NULL);
}

static bytes_t rt_bytes_from_literal(ctx_t* ctx, const uint8_t* ptr, uint32_t len) {
  bytes_t out = rt_bytes_alloc(ctx, len);
  if (len != 0) {
    memcpy(out.ptr, ptr, len);
    rt_mem_on_memcpy(ctx, len);
  }
  return out;
}

static bytes_view_t rt_view_from_literal(ctx_t* ctx, const uint8_t* ptr, uint32_t len) {
  if (len == 0) return rt_view_empty(ctx);
  bytes_view_t out;
  out.ptr = (uint8_t*)ptr;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  uint32_t off = 0;
  uint64_t aid = rt_dbg_alloc_try_find(ctx, (uint8_t*)ptr, len, &off);
  if (aid == 0) {
    aid = rt_dbg_alloc_register(ctx, (uint8_t*)ptr, len);
    off = 0;
  }
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#endif
  return out;
}

static bytes_t rt_bytes_clone(ctx_t* ctx, bytes_t src) {
  bytes_t out = rt_bytes_alloc(ctx, src.len);
  if (src.len != 0) {
    memcpy(out.ptr, src.ptr, src.len);
    rt_mem_on_memcpy(ctx, src.len);
  }
  return out;
}

static void rt_bytes_drop(ctx_t* ctx, bytes_t* b) {
  if (!b) return;
  if (b->len == 0) {
    b->ptr = ctx->heap.mem;
    return;
  }
#ifdef X07_DEBUG_BORROW
  uint64_t aid = rt_dbg_alloc_find(ctx, b->ptr, b->len, NULL);
  rt_dbg_alloc_kill(ctx, aid);
#endif
  uint32_t size = b->len;
  // Heap allocator stores the requested size in the allocation header; use it for exact accounting.
  if (ctx->allocator.free == rt_default_free) {
    heap_hdr_t* h = (heap_hdr_t*)(b->ptr - (uint32_t)sizeof(heap_hdr_t));
    if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("bytes.drop corrupt header");
    if (h->req_size == 0) rt_trap("bytes.drop corrupt header");
    size = h->req_size;
  }
  rt_free(ctx, b->ptr, size, 1);
  b->ptr = ctx->heap.mem;
  b->len = UINT32_C(0);
}

static void rt_task_out_drop(ctx_t* ctx, rt_task_out_t* out) {
  if (!out) return;
  if (out->kind == RT_TASK_OUT_KIND_BYTES) {
    rt_bytes_drop(ctx, &out->payload.bytes);
    out->payload.bytes = rt_bytes_empty(ctx);
    out->kind = RT_TASK_OUT_KIND_BYTES;
    return;
  }
  if (out->kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
    if (out->payload.result_bytes.tag) {
      rt_bytes_drop(ctx, &out->payload.result_bytes.payload.ok);
    }
    out->payload.result_bytes.tag = UINT32_C(0);
    out->payload.result_bytes.payload.err = UINT32_C(0);
    out->kind = RT_TASK_OUT_KIND_BYTES;
    out->payload.bytes = rt_bytes_empty(ctx);
    return;
  }
  rt_trap("task.out.drop invalid kind");
}

static bytes_t rt_view_to_bytes(ctx_t* ctx, bytes_view_t v) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, v.bid, v.off_bytes, v.len);
#endif
  bytes_t out = rt_bytes_alloc(ctx, v.len);
  if (v.len != 0) {
    memcpy(out.ptr, v.ptr, v.len);
    rt_mem_on_memcpy(ctx, v.len);
  }
  return out;
}

#ifdef X07_DEBUG_BORROW
static uint32_t rt_dbg_bytes_check(ctx_t* ctx, bytes_t b) {
  if (b.len == 0) return 1;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, NULL);
  if (aid == 0) return 0;
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aid - 1];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return 1;
}
#endif

static uint32_t rt_bytes_get_u8(ctx_t* ctx, bytes_t b, uint32_t idx) {
  if (idx >= b.len) rt_trap("bytes.get_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, b)) return 0;
#else
  (void)ctx;
#endif
  return (uint32_t)b.ptr[idx];
}

static bytes_t rt_bytes_set_u8(ctx_t* ctx, bytes_t b, uint32_t idx, uint32_t v) {
  if (idx >= b.len) rt_trap("bytes.set_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, b)) return b;
#else
  (void)ctx;
#endif
  b.ptr[idx] = (uint8_t)(v & UINT32_C(0xFF));
  return b;
}

static bytes_t rt_bytes_copy(ctx_t* ctx, bytes_t src, bytes_t dst) {
  if (dst.len < src.len) rt_trap("bytes.copy dst too small");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, src)) return dst;
  if (!rt_dbg_bytes_check(ctx, dst)) return dst;
#endif
  if (src.len != 0) {
    memcpy(dst.ptr, src.ptr, src.len);
    rt_mem_on_memcpy(ctx, src.len);
  }
  return dst;
}

static bytes_t rt_bytes_slice(ctx_t* ctx, bytes_t b, uint32_t start, uint32_t len) {
  if (start > b.len) rt_trap("bytes.slice oob");
  if (len > b.len - start) rt_trap("bytes.slice oob");

#ifdef X07_DEBUG_BORROW
  // Bulk ops bypass per-byte checks; validate once up front.
  uint32_t ok = rt_dbg_bytes_check(ctx, b);
  if (!ok && b.len != 0) {
    return rt_bytes_alloc(ctx, len);
  }
#endif

  bytes_t out = rt_bytes_alloc(ctx, len);
  if (len != 0) {
    memcpy(out.ptr, b.ptr + start, len);
    rt_mem_on_memcpy(ctx, len);
  }
  return out;
}

static bytes_t rt_bytes_concat(ctx_t* ctx, bytes_t a, bytes_t b) {
  uint32_t la = a.len;
  uint32_t lb = b.len;
  if (UINT32_MAX - la < lb) rt_trap("bytes.concat overflow");
#ifdef X07_DEBUG_BORROW
  uint32_t ok_a = rt_dbg_bytes_check(ctx, a);
  uint32_t ok_b = rt_dbg_bytes_check(ctx, b);
  if ((!ok_a && la != 0) || (!ok_b && lb != 0)) {
    return rt_bytes_alloc(ctx, la + lb);
  }
#endif

  bytes_t out = rt_bytes_alloc(ctx, la + lb);
  if (la != 0) {
    memcpy(out.ptr, a.ptr, la);
    rt_mem_on_memcpy(ctx, la);
  }
  if (lb != 0) {
    memcpy(out.ptr + la, b.ptr, lb);
    rt_mem_on_memcpy(ctx, lb);
  }
  return out;
}

static uint32_t rt_bytes_eq(ctx_t* ctx, bytes_t a, bytes_t b) {
  uint32_t a_prefix_len = 0;
  uint32_t b_prefix_len = 0;
  ctx->last_bytes_eq_valid = 0;
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, a)) return UINT32_C(0);
  if (!rt_dbg_bytes_check(ctx, b)) return UINT32_C(0);
#endif
  if (a.len != b.len) goto mismatch;
  if (a.len == 0) return UINT32_C(1);
#ifdef X07_DEBUG_BORROW
  // rt_dbg_bytes_check already ran above.
#endif
  if (memcmp(a.ptr, b.ptr, a.len) == 0) return UINT32_C(1);

mismatch:
  ctx->last_bytes_eq_valid = 1;
  ctx->last_bytes_eq_a_len = a.len;
  ctx->last_bytes_eq_b_len = b.len;
  a_prefix_len =
      (a.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? a.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
  b_prefix_len =
      (b.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? b.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
  if (a_prefix_len) memcpy(ctx->last_bytes_eq_a_prefix, a.ptr, a_prefix_len);
  if (b_prefix_len) memcpy(ctx->last_bytes_eq_b_prefix, b.ptr, b_prefix_len);
  return UINT32_C(0);
}

static uint32_t rt_bytes_cmp_range(
    ctx_t* ctx,
    bytes_t a,
    uint32_t a_off,
    uint32_t a_len,
    bytes_t b,
    uint32_t b_off,
    uint32_t b_len
) {
  if (a_off > a.len || a.len - a_off < a_len) rt_trap("bytes.cmp_range oob");
  if (b_off > b.len || b.len - b_off < b_len) rt_trap("bytes.cmp_range oob");

#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, a)) return UINT32_C(0);
  if (!rt_dbg_bytes_check(ctx, b)) return UINT32_C(0);
#else
  (void)ctx;
#endif

  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a.ptr + a_off, b.ptr + b_off, m);
    if (cmp < 0) return UINT32_MAX;
    if (cmp > 0) return UINT32_C(1);
  }
  if (a_len < b_len) return UINT32_MAX;
  if (a_len > b_len) return UINT32_C(1);
  return UINT32_C(0);
}

static bytes_view_t rt_bytes_view(ctx_t* ctx, bytes_t b) {
  bytes_view_t out;
  out.len = b.len;
#ifdef X07_DEBUG_BORROW
  if (b.len == 0) return rt_view_empty(ctx);
  uint32_t off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, &off);
  out.ptr = b.ptr;
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#else
  out.ptr = (b.len == 0) ? ctx->heap.mem : b.ptr;
#endif
  return out;
}

static bytes_view_t rt_bytes_subview(ctx_t* ctx, bytes_t b, uint32_t start, uint32_t len) {
  if (start > b.len) rt_trap("bytes.subview oob");
  if (len > b.len - start) rt_trap("bytes.subview oob");
  bytes_view_t out;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  if (len == 0) {
    return rt_view_empty(ctx);
  }
  uint32_t base_off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, &base_off);
  uint32_t off = base_off + start;
  out.ptr = b.ptr + start;
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#else
  out.ptr = (len == 0) ? ctx->heap.mem : (b.ptr + start);
#endif
  return out;
}

static uint32_t rt_view_get_u8(ctx_t* ctx, bytes_view_t v, uint32_t idx) {
  if (idx >= v.len) rt_trap("view.get_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, v.bid, v.off_bytes + idx, 1)) return 0;
#else
  (void)ctx;
#endif
  return (uint32_t)v.ptr[idx];
}

static bytes_view_t rt_view_slice(ctx_t* ctx, bytes_view_t v, uint32_t start, uint32_t len) {
  if (start > v.len) rt_trap("view.slice oob");
  if (len > v.len - start) rt_trap("view.slice oob");
  bytes_view_t out;
  out.ptr = v.ptr + start;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  out.aid = v.aid;
  out.bid = v.bid;
  out.off_bytes = v.off_bytes + start;
#else
  (void)ctx;
#endif
  return out;
}

static uint32_t rt_view_eq(ctx_t* ctx, bytes_view_t a, bytes_view_t b) {
  uint32_t a_prefix_len = 0;
  uint32_t b_prefix_len = 0;
  ctx->last_bytes_eq_valid = 0;
  if (a.len != b.len) goto mismatch;
  if (a.len == 0) return UINT32_C(1);
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, a.bid, a.off_bytes, a.len)) return UINT32_C(0);
  if (!rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len)) return UINT32_C(0);
#endif
  if (memcmp(a.ptr, b.ptr, a.len) == 0) return UINT32_C(1);

mismatch:
  a_prefix_len =
      (a.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? a.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
  b_prefix_len =
      (b.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? b.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
#ifdef X07_DEBUG_BORROW
  if (a_prefix_len && !rt_dbg_borrow_check(ctx, a.bid, a.off_bytes, a_prefix_len)) return UINT32_C(0);
  if (b_prefix_len && !rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b_prefix_len)) return UINT32_C(0);
#endif
  ctx->last_bytes_eq_valid = 1;
  ctx->last_bytes_eq_a_len = a.len;
  ctx->last_bytes_eq_b_len = b.len;
  if (a_prefix_len) memcpy(ctx->last_bytes_eq_a_prefix, a.ptr, a_prefix_len);
  if (b_prefix_len) memcpy(ctx->last_bytes_eq_b_prefix, b.ptr, b_prefix_len);
  return UINT32_C(0);
}

static uint32_t rt_view_cmp_range(
    ctx_t* ctx,
    bytes_view_t a,
    uint32_t a_off,
    uint32_t a_len,
    bytes_view_t b,
    uint32_t b_off,
    uint32_t b_len
) {
  if (a_off > a.len || a.len - a_off < a_len) rt_trap("view.cmp_range oob");
  if (b_off > b.len || b.len - b_off < b_len) rt_trap("view.cmp_range oob");

#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, a.bid, a.off_bytes + a_off, a_len)) return UINT32_C(0);
  if (!rt_dbg_borrow_check(ctx, b.bid, b.off_bytes + b_off, b_len)) return UINT32_C(0);
#else
  (void)ctx;
#endif

  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a.ptr + a_off, b.ptr + b_off, m);
    if (cmp < 0) return UINT32_MAX;
    if (cmp > 0) return UINT32_C(1);
  }
  if (a_len < b_len) return UINT32_MAX;
  if (a_len > b_len) return UINT32_C(1);
  return UINT32_C(0);
}

// Phase G2: deterministic single-thread cooperative scheduler + channels.

#define RT_WAIT_NONE UINT32_C(0)
#define RT_WAIT_JOIN UINT32_C(1)
#define RT_WAIT_SLEEP UINT32_C(2)
#define RT_WAIT_CHAN_SEND UINT32_C(3)
#define RT_WAIT_CHAN_RECV UINT32_C(4)
#define RT_WAIT_OS_PROC_JOIN UINT32_C(5)
#define RT_WAIT_OS_PROC_EXIT UINT32_C(6)

#define RT_TRACE_SWITCH UINT64_C(1)
#define RT_TRACE_BLOCK UINT64_C(2)
#define RT_TRACE_WAKE UINT64_C(3)
#define RT_TRACE_COMPLETE UINT64_C(4)

struct rt_task_s {
  uint32_t alive;
  uint32_t done;
  uint32_t canceled;

  uint32_t in_ready;
  uint32_t ready_next;

  uint32_t wait_kind;
  uint32_t wait_id;
  uint32_t wait_next;

  uint32_t join_wait_head;
  uint32_t join_wait_tail;

  uint32_t (*poll)(ctx_t* ctx, void* fut, rt_task_out_t* out);
  void (*drop)(ctx_t* ctx, void* fut);
  void* fut;
  rt_task_out_t out;
  uint32_t out_taken;
};

struct rt_timer_ev_s {
  uint64_t wake_time;
  uint64_t seq;
  uint32_t task_id;
};

struct rt_chan_bytes_s {
  uint32_t alive;
  uint32_t closed;
  uint32_t cap;

  bytes_t* buf;
  uint32_t head;
  uint32_t tail;
  uint32_t len;

  uint32_t send_wait_head;
  uint32_t send_wait_tail;
  uint32_t recv_wait_head;
  uint32_t recv_wait_tail;
};

struct rt_select_evt_s {
  uint32_t alive;
  uint32_t taken;

  uint64_t scope_key;

  uint32_t tag;
  uint32_t case_index;
  uint32_t src_id;

  bytes_t payload;
};

// Phase G2: deterministic streaming I/O reader handles + BufRead-like buffering.

#define RT_IO_READER_KIND_FILE UINT32_C(1)
#define RT_IO_READER_KIND_BYTES UINT32_C(2)

struct rt_io_reader_s {
  uint32_t alive;
  uint32_t kind;
  uint32_t eof;
  uint32_t pending_ticks;

#if X07_ENABLE_STREAMING_FILE_IO
  FILE* f;
#endif

  bytes_t bytes;
  uint32_t pos;
};

struct rt_bufread_s {
  uint32_t alive;
  iface_t reader;
  uint32_t eof;
  uint32_t direct_bytes;

  bytes_t buf;
  uint32_t start;
  uint32_t end;
};

static void rt_sched_trace_init(ctx_t* ctx) {
  if (ctx->sched_stats.sched_trace_hash == 0) {
    ctx->sched_stats.sched_trace_hash = UINT64_C(1469598103934665603);
  }
}

static void rt_sched_trace_u64(ctx_t* ctx, uint64_t x) {
  rt_sched_trace_init(ctx);
  ctx->sched_stats.sched_trace_hash ^= x;
  ctx->sched_stats.sched_trace_hash *= UINT64_C(1099511628211);
}

static void rt_sched_trace_event(ctx_t* ctx, uint64_t tag, uint64_t a, uint64_t b) {
  rt_sched_trace_u64(ctx, tag);
  rt_sched_trace_u64(ctx, a);
  rt_sched_trace_u64(ctx, b);
}

static rt_task_t* rt_task_ptr(ctx_t* ctx, uint32_t task_id) {
  if (task_id == 0 || task_id > ctx->sched_tasks_len) rt_trap("task invalid handle");
  rt_task_t* t = &ctx->sched_tasks[task_id - 1];
  if (!t->alive) rt_trap("task invalid handle");
  return t;
}

static rt_chan_bytes_t* rt_chan_bytes_ptr(ctx_t* ctx, uint32_t chan_id) {
  if (chan_id == 0 || chan_id > ctx->sched_chans_len) rt_trap("chan invalid handle");
  rt_chan_bytes_t* c = &ctx->sched_chans[chan_id - 1];
  if (!c->alive) rt_trap("chan invalid handle");
  return c;
}

static rt_select_evt_t* rt_select_evt_ptr(ctx_t* ctx, uint32_t evt_id) {
  if (evt_id == 0 || evt_id > ctx->sched_select_evts_len) rt_trap("X07T_SELECT_EVT_INVALID");
  rt_select_evt_t* e = &ctx->sched_select_evts[evt_id - 1];
  if (!e->alive) rt_trap("X07T_SELECT_EVT_INVALID");
  return e;
}

static rt_io_reader_t* rt_io_reader_ptr(ctx_t* ctx, uint32_t reader_id) {
  if (reader_id == 0 || reader_id > ctx->io_readers_len) rt_trap("io.reader invalid handle");
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  if (!r->alive) rt_trap("io.reader invalid handle");
  return r;
}

static rt_bufread_t* rt_bufread_ptr(ctx_t* ctx, uint32_t br_id) {
  if (br_id == 0 || br_id > ctx->bufreads_len) rt_trap("bufread invalid handle");
  rt_bufread_t* br = &ctx->bufreads[br_id - 1];
  if (!br->alive) rt_trap("bufread invalid handle");
  return br;
}

static void rt_sched_tasks_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_tasks_cap) return;
  rt_task_t* old_items = ctx->sched_tasks;
  uint32_t old_cap = ctx->sched_tasks_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_task_t);
  uint32_t new_cap = ctx->sched_tasks_cap ? ctx->sched_tasks_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_task_t* items = (rt_task_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_task_t),
    (uint32_t)_Alignof(rt_task_t)
  );
  if (old_items && ctx->sched_tasks_len) {
    uint32_t bytes = ctx->sched_tasks_len * (uint32_t)sizeof(rt_task_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_task_t));
  }
  ctx->sched_tasks = items;
  ctx->sched_tasks_cap = new_cap;
}

static void rt_sched_chans_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_chans_cap) return;
  rt_chan_bytes_t* old_items = ctx->sched_chans;
  uint32_t old_cap = ctx->sched_chans_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_chan_bytes_t);
  uint32_t new_cap = ctx->sched_chans_cap ? ctx->sched_chans_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_chan_bytes_t* items = (rt_chan_bytes_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_chan_bytes_t),
    (uint32_t)_Alignof(rt_chan_bytes_t)
  );
  if (old_items && ctx->sched_chans_len) {
    uint32_t bytes = ctx->sched_chans_len * (uint32_t)sizeof(rt_chan_bytes_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_chan_bytes_t));
  }
  ctx->sched_chans = items;
  ctx->sched_chans_cap = new_cap;
}

static void rt_sched_select_evts_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_select_evts_cap) return;
  rt_select_evt_t* old_items = ctx->sched_select_evts;
  uint32_t old_cap = ctx->sched_select_evts_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_select_evt_t);
  uint32_t new_cap = ctx->sched_select_evts_cap ? ctx->sched_select_evts_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_select_evt_t* items = (rt_select_evt_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_select_evt_t),
    (uint32_t)_Alignof(rt_select_evt_t)
  );
  if (old_items && ctx->sched_select_evts_len) {
    uint32_t bytes = ctx->sched_select_evts_len * (uint32_t)sizeof(rt_select_evt_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_select_evt_t));
  }
  ctx->sched_select_evts = items;
  ctx->sched_select_evts_cap = new_cap;
}

static void rt_sched_timers_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_timers_cap) return;
  rt_timer_ev_t* old_items = ctx->sched_timers;
  uint32_t old_cap = ctx->sched_timers_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_timer_ev_t);
  uint32_t new_cap = ctx->sched_timers_cap ? ctx->sched_timers_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_timer_ev_t* items = (rt_timer_ev_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_timer_ev_t),
    (uint32_t)_Alignof(rt_timer_ev_t)
  );
  if (old_items && ctx->sched_timers_len) {
    uint32_t bytes = ctx->sched_timers_len * (uint32_t)sizeof(rt_timer_ev_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_timer_ev_t));
  }
  ctx->sched_timers = items;
  ctx->sched_timers_cap = new_cap;
}

static void rt_io_readers_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->io_readers_cap) return;
  rt_io_reader_t* old_items = ctx->io_readers;
  uint32_t old_cap = ctx->io_readers_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_io_reader_t);
  uint32_t new_cap = ctx->io_readers_cap ? ctx->io_readers_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_io_reader_t* items = (rt_io_reader_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_io_reader_t),
    (uint32_t)_Alignof(rt_io_reader_t)
  );
  if (old_items && ctx->io_readers_len) {
    uint32_t bytes = ctx->io_readers_len * (uint32_t)sizeof(rt_io_reader_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_io_reader_t));
  }
  ctx->io_readers = items;
  ctx->io_readers_cap = new_cap;
}

static void rt_bufreads_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->bufreads_cap) return;
  rt_bufread_t* old_items = ctx->bufreads;
  uint32_t old_cap = ctx->bufreads_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_bufread_t);
  uint32_t new_cap = ctx->bufreads_cap ? ctx->bufreads_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_bufread_t* items = (rt_bufread_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_bufread_t),
    (uint32_t)_Alignof(rt_bufread_t)
  );
  if (old_items && ctx->bufreads_len) {
    uint32_t bytes = ctx->bufreads_len * (uint32_t)sizeof(rt_bufread_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_bufread_t));
  }
  ctx->bufreads = items;
  ctx->bufreads_cap = new_cap;
}

static void rt_ready_push(ctx_t* ctx, uint32_t task_id) {
  if (task_id == 0) return;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return;
  if (t->in_ready) return;
  t->in_ready = 1;
  t->ready_next = 0;
  if (ctx->sched_ready_tail == 0) {
    ctx->sched_ready_head = task_id;
    ctx->sched_ready_tail = task_id;
    return;
  }
  rt_task_t* tail = rt_task_ptr(ctx, ctx->sched_ready_tail);
  tail->ready_next = task_id;
  ctx->sched_ready_tail = task_id;
}

static uint32_t rt_ready_pop(ctx_t* ctx) {
  for (;;) {
    uint32_t task_id = ctx->sched_ready_head;
    if (task_id == 0) return 0;
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    ctx->sched_ready_head = t->ready_next;
    if (ctx->sched_ready_head == 0) ctx->sched_ready_tail = 0;
    t->ready_next = 0;
    t->in_ready = 0;
    if (t->done) continue;
    return task_id;
  }
}

static void rt_wait_list_push(ctx_t* ctx, uint32_t* head, uint32_t* tail, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  t->wait_next = 0;
  if (*tail == 0) {
    *head = task_id;
    *tail = task_id;
    return;
  }
  rt_task_t* last = rt_task_ptr(ctx, *tail);
  last->wait_next = task_id;
  *tail = task_id;
}

static uint32_t rt_wait_list_pop(ctx_t* ctx, uint32_t* head, uint32_t* tail) {
  for (;;) {
    uint32_t task_id = *head;
    if (task_id == 0) return 0;
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    *head = t->wait_next;
    if (*head == 0) *tail = 0;
    t->wait_next = 0;
    if (t->done) continue;
    return task_id;
  }
}

static void rt_sched_wake(ctx_t* ctx, uint32_t task_id, uint32_t reason_kind, uint32_t reason_id) {
  if (task_id == 0) return;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return;
  t->wait_kind = RT_WAIT_NONE;
  t->wait_id = 0;
  rt_ready_push(ctx, task_id);
  ctx->sched_stats.wake_events += 1;
  rt_sched_trace_event(
    ctx,
    RT_TRACE_WAKE,
    (uint64_t)task_id,
    ((uint64_t)reason_kind << 32) | (uint64_t)reason_id
  );
}

static uint32_t rt_timer_less(rt_timer_ev_t a, rt_timer_ev_t b) {
  if (a.wake_time < b.wake_time) return 1;
  if (a.wake_time > b.wake_time) return 0;
  return (a.seq < b.seq) ? 1 : 0;
}

static void rt_timer_push(ctx_t* ctx, uint64_t wake_time, uint32_t task_id) {
  rt_sched_timers_ensure_cap(ctx, ctx->sched_timers_len + 1);
  uint32_t i = ctx->sched_timers_len++;
  rt_timer_ev_t ev;
  ev.wake_time = wake_time;
  ev.seq = ctx->sched_seq++;
  ev.task_id = task_id;

  while (i > 0) {
    uint32_t p = (i - 1) / 2;
    rt_timer_ev_t parent = ctx->sched_timers[p];
    if (!rt_timer_less(ev, parent)) break;
    ctx->sched_timers[i] = parent;
    i = p;
  }
  ctx->sched_timers[i] = ev;
}

static uint32_t rt_timer_pop(ctx_t* ctx, rt_timer_ev_t* out) {
  if (ctx->sched_timers_len == 0) return 0;
  if (out) *out = ctx->sched_timers[0];
  ctx->sched_timers_len -= 1;
  if (ctx->sched_timers_len == 0) return 1;

  rt_timer_ev_t ev = ctx->sched_timers[ctx->sched_timers_len];
  uint32_t i = 0;
  for (;;) {
    uint32_t l = i * 2 + 1;
    uint32_t r = l + 1;
    if (l >= ctx->sched_timers_len) break;
    uint32_t m = l;
    if (r < ctx->sched_timers_len && rt_timer_less(ctx->sched_timers[r], ctx->sched_timers[l])) {
      m = r;
    }
    if (!rt_timer_less(ctx->sched_timers[m], ev)) break;
    ctx->sched_timers[i] = ctx->sched_timers[m];
    i = m;
  }
  ctx->sched_timers[i] = ev;
  return 1;
}

static uint64_t rt_timer_peek_wake(ctx_t* ctx) {
  if (ctx->sched_timers_len == 0) return UINT64_MAX;
  return ctx->sched_timers[0].wake_time;
}

static uint32_t rt_sched_step(ctx_t* ctx) {
  uint32_t task_id = rt_ready_pop(ctx);
  if (task_id != 0) {
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    if (t->done) return UINT32_C(1);

    ctx->sched_stats.ctx_switches += 1;
    rt_sched_trace_event(ctx, RT_TRACE_SWITCH, (uint64_t)task_id, ctx->sched_now_ticks);

    uint32_t prev = ctx->sched_current_task;
    ctx->sched_current_task = task_id;

    rt_task_out_t out = rt_task_out_empty(ctx);
    uint32_t done = t->poll(ctx, t->fut, &out);

    ctx->sched_current_task = prev;
    t = rt_task_ptr(ctx, task_id);

    if (done) {
      t->done = 1;
      t->out = out;
      t->out_taken = 0;
      if (t->drop && t->fut) {
        t->drop(ctx, t->fut);
      }
      t->drop = NULL;
      t->fut = NULL;
      rt_sched_trace_event(ctx, RT_TRACE_COMPLETE, (uint64_t)task_id, ctx->sched_now_ticks);

      uint32_t w = t->join_wait_head;
      uint32_t wt = t->join_wait_tail;
      (void)wt;
      t->join_wait_head = 0;
      t->join_wait_tail = 0;
      while (w != 0) {
        rt_task_t* waiter = rt_task_ptr(ctx, w);
        uint32_t next = waiter->wait_next;
        waiter->wait_next = 0;
        rt_sched_wake(ctx, w, RT_WAIT_JOIN, task_id);
        w = next;
      }
      return UINT32_C(1);
    }

    if (!t->in_ready && t->wait_kind == RT_WAIT_NONE) {
      const char* dbg = getenv("X07_DEBUG_SCHED");
      if (dbg && dbg[0] == '1' && dbg[1] == '\0') {
        char msg[192];
        uint32_t st = 0;
        if (t->fut) memcpy(&st, t->fut, sizeof(uint32_t));
        (void)snprintf(
          msg,
          sizeof(msg),
          "task pending without block (task_id=%u poll=%p state=%u)",
          (unsigned)task_id,
          (void*)t->poll,
          (unsigned)st
        );
        rt_trap(msg);
      }
      char msg[96];
      (void)snprintf(
        msg,
        sizeof(msg),
        "task pending without block (task_id=%u)",
        (unsigned)task_id
      );
      rt_trap(msg);
    }
    (void)rt_os_process_poll_all(ctx, 0);
    return UINT32_C(1);
  }

  rt_timer_ev_t ev;
  while (rt_timer_pop(ctx, &ev)) {
    if (ev.task_id == 0) continue;
    rt_task_t* t = rt_task_ptr(ctx, ev.task_id);
    if (t->done) continue;
    if (ev.wake_time > ctx->sched_now_ticks) ctx->sched_now_ticks = ev.wake_time;
    ctx->sched_stats.virtual_time_end = ctx->sched_now_ticks;
    rt_sched_wake(ctx, ev.task_id, RT_WAIT_SLEEP, 0);
    return UINT32_C(1);
  }

  if (rt_os_process_poll_all(ctx, 50)) return UINT32_C(1);
  return UINT32_C(0);
}

static __attribute__((noreturn)) void rt_sched_deadlock(void) {
  rt_trap("scheduler deadlock");
}

static uint32_t rt_task_create(
    ctx_t* ctx,
    uint32_t (*poll)(ctx_t* ctx, void* fut, rt_task_out_t* out),
    void (*drop)(ctx_t* ctx, void* fut),
    void* fut
) {
  rt_sched_tasks_ensure_cap(ctx, ctx->sched_tasks_len + 1);
  uint32_t task_id = ctx->sched_tasks_len + 1;
  rt_task_t* t = &ctx->sched_tasks[task_id - 1];
  memset(t, 0, sizeof(*t));
  t->alive = 1;
  t->poll = poll;
  t->drop = drop;
  t->fut = fut;
  t->out = rt_task_out_empty(ctx);
  t->out_taken = 0;
  ctx->sched_tasks_len += 1;

  ctx->sched_stats.tasks_spawned += 1;
  rt_ready_push(ctx, task_id);
  return task_id;
}

static uint32_t rt_task_spawn(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.spawn_calls += 1;
  (void)rt_task_ptr(ctx, task_id);
  return task_id;
}

static uint32_t rt_task_cancel(ctx_t* ctx, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return UINT32_C(0);
  t->canceled = 1;
  t->done = 1;
  if (t->drop && t->fut) {
    t->drop(ctx, t->fut);
  }
  t->drop = NULL;
  t->fut = NULL;
  rt_task_out_drop(ctx, &t->out);
  t->out = rt_task_out_empty(ctx);
  t->out_taken = 0;
  rt_sched_trace_event(ctx, RT_TRACE_COMPLETE, (uint64_t)task_id, ctx->sched_now_ticks);

  uint32_t w = t->join_wait_head;
  t->join_wait_head = 0;
  t->join_wait_tail = 0;
  while (w != 0) {
    rt_task_t* waiter = rt_task_ptr(ctx, w);
    uint32_t next = waiter->wait_next;
    waiter->wait_next = 0;
    rt_sched_wake(ctx, w, RT_WAIT_JOIN, task_id);
    w = next;
  }
  return UINT32_C(1);
}

static uint32_t rt_task_join_bytes_poll(ctx_t* ctx, uint32_t task_id, bytes_t* out) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) {
    if (t->out_taken) rt_trap("join already taken");
    t->out_taken = 1;
    if (t->canceled) {
      if (out) *out = rt_bytes_empty(ctx);
      t->out = rt_task_out_empty(ctx);
      return UINT32_C(1);
    }
    if (t->out.kind != RT_TASK_OUT_KIND_BYTES) rt_trap("task.join.bytes kind mismatch");
    if (out) {
      *out = t->out.payload.bytes;
    } else {
      rt_bytes_drop(ctx, &t->out.payload.bytes);
    }
    t->out = rt_task_out_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("join.poll from main");
  if (cur == task_id) rt_trap("join self");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_JOIN && me->wait_id == task_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("join while already waiting");

  me->wait_kind = RT_WAIT_JOIN;
  me->wait_id = task_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_JOIN << 32) | task_id);
  rt_wait_list_push(ctx, &t->join_wait_head, &t->join_wait_tail, cur);
  return UINT32_C(0);
}

static bytes_t rt_task_join_bytes_block(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  while (!t->done) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    t = rt_task_ptr(ctx, task_id);
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) return rt_bytes_empty(ctx);
  if (t->out.kind != RT_TASK_OUT_KIND_BYTES) rt_trap("task.join.bytes kind mismatch");
  bytes_t out_b = t->out.payload.bytes;
  t->out = rt_task_out_empty(ctx);
  return out_b;
}

static uint32_t rt_task_is_finished(ctx_t* ctx, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  return t->done ? UINT32_C(1) : UINT32_C(0);
}

static result_bytes_t rt_task_try_join_bytes(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (!t->done) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  if (t->out.kind != RT_TASK_OUT_KIND_BYTES) rt_trap("task.try_join.bytes kind mismatch");
  bytes_t out_b = t->out.payload.bytes;
  t->out = rt_task_out_empty(ctx);
  return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out_b };
}

static uint32_t rt_task_join_result_bytes_poll(ctx_t* ctx, uint32_t task_id, result_bytes_t* out) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) {
    if (t->out_taken) rt_trap("join already taken");
    t->out_taken = 1;
    if (t->canceled) {
      if (out) *out = (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
      t->out = rt_task_out_empty(ctx);
      return UINT32_C(1);
    }
    if (t->out.kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("task.join.result_bytes kind mismatch");
    if (out) {
      *out = t->out.payload.result_bytes;
    } else {
      if (t->out.payload.result_bytes.tag) {
        rt_bytes_drop(ctx, &t->out.payload.result_bytes.payload.ok);
      }
    }
    t->out = rt_task_out_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("join.poll from main");
  if (cur == task_id) rt_trap("join self");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_JOIN && me->wait_id == task_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("join while already waiting");

  me->wait_kind = RT_WAIT_JOIN;
  me->wait_id = task_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_JOIN << 32) | task_id);
  rt_wait_list_push(ctx, &t->join_wait_head, &t->join_wait_tail, cur);
  return UINT32_C(0);
}

static result_bytes_t rt_task_join_result_bytes_block(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  while (!t->done) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    t = rt_task_ptr(ctx, task_id);
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  if (t->out.kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("task.join.result_bytes kind mismatch");
  result_bytes_t out_rb = t->out.payload.result_bytes;
  t->out = rt_task_out_empty(ctx);
  return out_rb;
}

static result_result_bytes_t rt_task_try_join_result_bytes(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (!t->done) {
    return (result_result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) {
    return (result_result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  if (t->out.kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("task.try_join.result_bytes kind mismatch");
  result_bytes_t out_rb = t->out.payload.result_bytes;
  t->out = rt_task_out_empty(ctx);
  return (result_result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out_rb };
}

static void rt_task_yield(ctx_t* ctx) {
  ctx->sched_stats.yield_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("yield from main");
  rt_task_t* me = rt_task_ptr(ctx, cur);
  me->wait_kind = RT_WAIT_NONE;
  me->wait_id = 0;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_NONE << 32) | RT_WAIT_NONE);
  rt_ready_push(ctx, cur);
}

static uint32_t rt_task_yield_block(ctx_t* ctx) {
  ctx->sched_stats.yield_calls += 1;
  (void)rt_sched_step(ctx);
  return UINT32_C(0);
}

static void rt_task_sleep(ctx_t* ctx, uint32_t ticks) {
  ctx->sched_stats.sleep_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("sleep from main");
  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (ticks == 0) {
    rt_ready_push(ctx, cur);
    return;
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("sleep while already waiting");
  me->wait_kind = RT_WAIT_SLEEP;
  me->wait_id = 0;
  ctx->sched_stats.blocked_waits += 1;
  uint64_t wake_time = ctx->sched_now_ticks + (uint64_t)ticks;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_SLEEP << 32) | (uint64_t)ticks);
  rt_timer_push(ctx, wake_time, cur);
}

static uint32_t rt_task_sleep_block(ctx_t* ctx, uint32_t ticks) {
  ctx->sched_stats.sleep_calls += 1;
  if (ticks == 0) return UINT32_C(0);
  uint64_t target = ctx->sched_now_ticks + (uint64_t)ticks;
  while (ctx->sched_now_ticks < target) {
    if (ctx->sched_ready_head != 0) {
      if (!rt_sched_step(ctx)) break;
      continue;
    }
    uint64_t next = rt_timer_peek_wake(ctx);
    if (next == UINT64_MAX || next > target) {
      ctx->sched_now_ticks = target;
      ctx->sched_stats.virtual_time_end = ctx->sched_now_ticks;
      break;
    }
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
  return UINT32_C(0);
}

typedef struct {
  uint32_t task_id;
  uint32_t kind;
  uint32_t state;
  uint32_t gen;
} rt_scope_slot_t;

typedef struct {
  uint32_t max_children;
  uint64_t max_ticks;
  uint64_t max_blocked_waits;
  uint64_t max_join_polls;
  uint32_t max_slot_result_bytes;

  uint64_t key;

  uint8_t cancel_requested;

  uint64_t snap_ticks;
  uint64_t snap_blocked_waits;
  uint64_t snap_join_polls;
  uint64_t snap_tasks_spawned;

  uint32_t child_cap;
  uint32_t child_len;
  uint32_t* child_task_ids;
  uint32_t* child_task_kinds;

  uint32_t reg_cap;
  uint32_t reg_len;
  uint32_t* reg_task_ids;

  uint32_t slot_cap;
  uint32_t slot_len;
  rt_scope_slot_t* slots;

  uint32_t join_phase;
  uint32_t join_slot_i;
  uint32_t join_child_i;

  uint32_t select_rr_cursor;
} rt_scope_t;

typedef struct {
  uint32_t active;
  uint32_t mode;
  uint32_t violated;
  uint32_t err_code;
  const uint8_t* label_ptr;
  uint32_t label_len;

  uint8_t yielded;
  uint8_t fuel_clamped;
  uint16_t reserved16;

  uint64_t max_alloc_bytes;
  uint64_t max_alloc_calls;
  uint64_t max_realloc_calls;
  uint64_t max_memcpy_bytes;
  uint64_t max_sched_ticks;
  uint64_t max_fuel;

  uint64_t snap_alloc_bytes;
  uint64_t snap_alloc_calls;
  uint64_t snap_realloc_calls;
  uint64_t snap_memcpy_bytes;
  uint64_t snap_sched_ticks;

  uint64_t snap_fuel_saved;
  uint64_t snap_fuel_start;
} rt_budget_scope_t;

#define RT_BUDGET_MODE_TRAP UINT32_C(0)
#define RT_BUDGET_MODE_RESULT_ERR UINT32_C(1)
#define RT_BUDGET_MODE_STATS_ONLY UINT32_C(2)
#define RT_BUDGET_MODE_YIELD UINT32_C(3)

#define RT_ERR_BUDGET_ALLOC_BYTES UINT32_C(0x80000001)
#define RT_ERR_BUDGET_ALLOC_CALLS UINT32_C(0x80000002)
#define RT_ERR_BUDGET_REALLOC_CALLS UINT32_C(0x80000003)
#define RT_ERR_BUDGET_MEMCPY_BYTES UINT32_C(0x80000004)
#define RT_ERR_BUDGET_SCHED_TICKS UINT32_C(0x80000005)

static void rt_budget_scope_drop(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return;
  if (s->fuel_clamped) {
    uint64_t consumed = s->snap_fuel_start - ctx->fuel;
    ctx->fuel = s->snap_fuel_saved - consumed;
    if (ctx->budget_fuel_depth == 0) rt_trap("budget fuel depth underflow");
    ctx->budget_fuel_depth -= 1;
  }
  s->active = UINT32_C(0);
}

static void rt_budget_scope_dispose_on_drop(ctx_t* ctx, rt_budget_scope_t* s) {
  rt_budget_scope_drop(ctx, s);
}

static void rt_budget_scope_init(
  ctx_t* ctx,
  rt_budget_scope_t* s,
  uint32_t mode,
  const uint8_t* label_ptr,
  uint32_t label_len,
  uint64_t max_alloc_bytes,
  uint64_t max_alloc_calls,
  uint64_t max_realloc_calls,
  uint64_t max_memcpy_bytes,
  uint64_t max_sched_ticks,
  uint64_t max_fuel
) {
  memset(s, 0, sizeof(*s));
  s->active = UINT32_C(1);
  s->mode = mode;
  s->label_ptr = label_ptr;
  s->label_len = label_len;
  s->max_alloc_bytes = max_alloc_bytes;
  s->max_alloc_calls = max_alloc_calls;
  s->max_realloc_calls = max_realloc_calls;
  s->max_memcpy_bytes = max_memcpy_bytes;
  s->max_sched_ticks = max_sched_ticks;
  s->max_fuel = max_fuel;

  s->snap_alloc_bytes = ctx->mem_stats.bytes_alloc_total;
  s->snap_alloc_calls = ctx->mem_stats.alloc_calls;
  s->snap_realloc_calls = ctx->mem_stats.realloc_calls;
  s->snap_memcpy_bytes = ctx->mem_stats.memcpy_bytes;
  s->snap_sched_ticks = ctx->sched_now_ticks;

  s->snap_fuel_saved = ctx->fuel;
  s->snap_fuel_start = ctx->fuel;
  if (max_fuel != 0 && ctx->fuel > max_fuel) {
    s->snap_fuel_start = max_fuel;
    ctx->fuel = max_fuel;
    s->fuel_clamped = 1;
    ctx->budget_fuel_depth += 1;
  }
}

static void rt_budget_scope_check_exit(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return;
  s->violated = UINT32_C(0);
  s->err_code = UINT32_C(0);

  uint64_t alloc_bytes = ctx->mem_stats.bytes_alloc_total - s->snap_alloc_bytes;
  if (s->max_alloc_bytes != 0 && alloc_bytes > s->max_alloc_bytes) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_ALLOC_BYTES;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_ALLOC_BYTES");
  }

  uint64_t alloc_calls = ctx->mem_stats.alloc_calls - s->snap_alloc_calls;
  if (s->max_alloc_calls != 0 && alloc_calls > s->max_alloc_calls) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_ALLOC_CALLS;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_ALLOC_CALLS");
  }

  uint64_t realloc_calls = ctx->mem_stats.realloc_calls - s->snap_realloc_calls;
  if (s->max_realloc_calls != 0 && realloc_calls > s->max_realloc_calls) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_REALLOC_CALLS;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_REALLOC_CALLS");
  }

  uint64_t memcpy_bytes = ctx->mem_stats.memcpy_bytes - s->snap_memcpy_bytes;
  if (s->max_memcpy_bytes != 0 && memcpy_bytes > s->max_memcpy_bytes) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_MEMCPY_BYTES;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_MEMCPY_BYTES");
  }

  uint64_t sched_ticks = ctx->sched_now_ticks - s->snap_sched_ticks;
  if (s->max_sched_ticks != 0 && sched_ticks > s->max_sched_ticks) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY || s->mode == RT_BUDGET_MODE_YIELD) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_SCHED_TICKS;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_SCHED_TICKS");
  }
}

static uint32_t rt_budget_scope_exit_poll(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return UINT32_C(1);

  if (s->mode == RT_BUDGET_MODE_YIELD && !s->yielded && s->max_sched_ticks != 0) {
    uint64_t ticks = ctx->sched_now_ticks - s->snap_sched_ticks;
    if (ticks > s->max_sched_ticks) {
      s->yielded = 1;
      rt_task_yield(ctx);
      s->snap_sched_ticks = ctx->sched_now_ticks;
      return UINT32_C(0);
    }
  }

  rt_budget_scope_check_exit(ctx, s);
  rt_budget_scope_drop(ctx, s);
  return UINT32_C(1);
}

static void rt_budget_scope_exit_block(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return;

  if (s->mode == RT_BUDGET_MODE_YIELD && !s->yielded && s->max_sched_ticks != 0) {
    uint64_t ticks = ctx->sched_now_ticks - s->snap_sched_ticks;
    if (ticks > s->max_sched_ticks) {
      s->yielded = 1;
      (void)rt_task_yield_block(ctx);
      s->snap_sched_ticks = ctx->sched_now_ticks;
    }
  }

  rt_budget_scope_check_exit(ctx, s);
  rt_budget_scope_drop(ctx, s);
}

#define RT_SCOPE_SLOT_EMPTY UINT32_C(0)
#define RT_SCOPE_SLOT_PENDING UINT32_C(1)
#define RT_SCOPE_SLOT_TAKEN UINT32_C(2)
#define RT_SCOPE_SLOT_CONSUMED UINT32_C(3)

static void rt_scope_init(
  ctx_t* ctx,
  rt_scope_t* s,
  uint32_t max_children,
  uint64_t max_ticks,
  uint64_t max_blocked_waits,
  uint64_t max_join_polls,
  uint32_t max_slot_result_bytes
) {
  memset(s, 0, sizeof(*s));
  s->max_children = max_children;
  s->max_ticks = max_ticks;
  s->max_blocked_waits = max_blocked_waits;
  s->max_join_polls = max_join_polls;
  s->max_slot_result_bytes = max_slot_result_bytes;
  s->key = (uint64_t)(uintptr_t)s;

  s->child_cap = max_children;
  s->reg_cap = max_children;
  s->slot_cap = max_children;
  s->slot_len = max_children;

  if (max_children != 0) {
    s->child_task_ids = (uint32_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
    s->child_task_kinds = (uint32_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
    s->reg_task_ids = (uint32_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
    s->slots = (rt_scope_slot_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(rt_scope_slot_t),
      (uint32_t)_Alignof(rt_scope_slot_t)
    );
    memset(s->slots, 0, max_children * (uint32_t)sizeof(rt_scope_slot_t));
  }

  s->snap_ticks = ctx->sched_now_ticks;
  s->snap_blocked_waits = ctx->sched_stats.blocked_waits;
  s->snap_join_polls = ctx->sched_stats.join_calls;
  s->snap_tasks_spawned = ctx->sched_stats.tasks_spawned;
}

static void rt_scope_drop(ctx_t* ctx, rt_scope_t* s) {
  if (s->key != 0 && ctx->sched_select_evts_len != 0) {
    for (uint32_t i = 0; i < ctx->sched_select_evts_len; i++) {
      rt_select_evt_t* e = &ctx->sched_select_evts[i];
      if (!e->alive) continue;
      if (e->scope_key != s->key) continue;
      rt_bytes_drop(ctx, &e->payload);
      e->payload = rt_bytes_empty(ctx);
      e->taken = 1;
      e->alive = 0;
    }
  }
  if (s->child_cap && s->child_task_ids) {
    rt_free(
      ctx,
      s->child_task_ids,
      s->child_cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (s->child_cap && s->child_task_kinds) {
    rt_free(
      ctx,
      s->child_task_kinds,
      s->child_cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (s->reg_cap && s->reg_task_ids) {
    rt_free(
      ctx,
      s->reg_task_ids,
      s->reg_cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (s->slot_cap && s->slots) {
    rt_free(
      ctx,
      s->slots,
      s->slot_cap * (uint32_t)sizeof(rt_scope_slot_t),
      (uint32_t)_Alignof(rt_scope_slot_t)
    );
  }
  memset(s, 0, sizeof(*s));
}

static void rt_scope_register_task(rt_scope_t* s, uint32_t task_id) {
  if (s->reg_len >= s->reg_cap) rt_trap("X07T_SCOPE_BUDGET_CHILDREN_EXCEEDED");
  s->reg_task_ids[s->reg_len] = task_id;
  s->reg_len += 1;
}

static void rt_scope_unregister_task(rt_scope_t* s, uint32_t task_id) {
  for (uint32_t i = 0; i < s->reg_len; i++) {
    if (s->reg_task_ids[i] != task_id) continue;
    for (uint32_t j = i + 1; j < s->reg_len; j++) {
      s->reg_task_ids[j - 1] = s->reg_task_ids[j];
    }
    s->reg_len -= 1;
    if (s->reg_len < s->reg_cap) s->reg_task_ids[s->reg_len] = 0;
    return;
  }
}

static uint32_t rt_scope_start_soon(ctx_t* ctx, rt_scope_t* s, uint32_t task_id, uint32_t kind) {
  (void)ctx;
  if (s->cancel_requested) rt_trap("X07T_SCOPE_START_AFTER_CANCEL");
  rt_scope_register_task(s, task_id);
  if (s->child_len >= s->child_cap) rt_trap("X07T_SCOPE_BUDGET_CHILDREN_EXCEEDED");
  s->child_task_ids[s->child_len] = task_id;
  s->child_task_kinds[s->child_len] = kind;
  s->child_len += 1;
  return UINT32_C(1);
}

static uint32_t rt_scope_async_let(ctx_t* ctx, rt_scope_t* s, uint32_t task_id, uint32_t kind) {
  (void)ctx;
  if (s->cancel_requested) rt_trap("X07T_SCOPE_START_AFTER_CANCEL");
  rt_scope_register_task(s, task_id);
  for (uint32_t i = 0; i < s->slot_len; i++) {
    rt_scope_slot_t* slot = &s->slots[i];
    if (slot->state != RT_SCOPE_SLOT_EMPTY && slot->state != RT_SCOPE_SLOT_CONSUMED) continue;
    slot->gen += 1;
    slot->task_id = task_id;
    slot->kind = kind;
    slot->state = RT_SCOPE_SLOT_PENDING;
    uint32_t handle = (slot->gen << 16) | i;
    return handle;
  }
  rt_trap("X07T_SCOPE_BUDGET_CHILDREN_EXCEEDED");
}

static uint32_t rt_scope_slot_is_finished(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state == RT_SCOPE_SLOT_PENDING || slot->state == RT_SCOPE_SLOT_TAKEN) {
    return rt_task_is_finished(ctx, slot->task_id);
  }
  if (slot->state == RT_SCOPE_SLOT_CONSUMED) return UINT32_C(1);
  rt_trap("X07T_SCOPE_SLOT_INVALID");
}

static uint32_t rt_scope_slot_task_for_await(
  rt_scope_t* s,
  uint32_t slot_id,
  uint32_t expected_kind
) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state == RT_SCOPE_SLOT_PENDING) {
    slot->state = RT_SCOPE_SLOT_TAKEN;
  } else if (slot->state != RT_SCOPE_SLOT_TAKEN) {
    rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  }
  if (slot->kind != expected_kind) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  if (slot->task_id == 0) rt_trap("X07T_SCOPE_SLOT_INVALID");
  return slot->task_id;
}

static void rt_scope_check_slot_result_size(rt_scope_t* s, bytes_t out) {
  if (s->max_slot_result_bytes != 0 && out.len > s->max_slot_result_bytes) {
    rt_trap("X07T_SCOPE_SLOT_RESULT_TOO_LARGE");
  }
}

static uint32_t rt_scope_await_slot_bytes_poll(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id, bytes_t* out) {
  uint32_t task_id = rt_scope_slot_task_for_await(s, slot_id, RT_TASK_OUT_KIND_BYTES);
  uint32_t done = rt_task_join_bytes_poll(ctx, task_id, out);
  if (!done) return UINT32_C(0);
  rt_scope_check_slot_result_size(s, *out);
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return UINT32_C(1);
}

static bytes_t rt_scope_await_slot_bytes_block(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  slot->state = RT_SCOPE_SLOT_TAKEN;
  uint32_t task_id = slot->task_id;
  bytes_t out = rt_task_join_bytes_block(ctx, task_id);
  rt_scope_check_slot_result_size(s, out);
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return out;
}

static uint32_t rt_scope_await_slot_result_bytes_poll(
  ctx_t* ctx,
  rt_scope_t* s,
  uint32_t slot_id,
  result_bytes_t* out
) {
  uint32_t task_id = rt_scope_slot_task_for_await(s, slot_id, RT_TASK_OUT_KIND_RESULT_BYTES);
  uint32_t done = rt_task_join_result_bytes_poll(ctx, task_id, out);
  if (!done) return UINT32_C(0);
  if (out->tag) rt_scope_check_slot_result_size(s, out->payload.ok);
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return UINT32_C(1);
}

static result_bytes_t rt_scope_await_slot_result_bytes_block(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  slot->state = RT_SCOPE_SLOT_TAKEN;
  uint32_t task_id = slot->task_id;
  result_bytes_t out = rt_task_join_result_bytes_block(ctx, task_id);
  if (out.tag) rt_scope_check_slot_result_size(s, out.payload.ok);
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return out;
}

static result_bytes_t rt_scope_try_await_slot_bytes(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  uint32_t task_id = slot->task_id;
  result_bytes_t r = rt_task_try_join_bytes(ctx, task_id);
  if (r.tag) {
    rt_scope_check_slot_result_size(s, r.payload.ok);
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
    return r;
  }
  if (r.payload.err == UINT32_C(2)) {
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
  }
  return r;
}

static result_result_bytes_t rt_scope_try_await_slot_result_bytes(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  uint32_t task_id = slot->task_id;
  result_result_bytes_t r = rt_task_try_join_result_bytes(ctx, task_id);
  if (r.tag) {
    if (r.payload.ok.tag) rt_scope_check_slot_result_size(s, r.payload.ok.payload.ok);
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
    return r;
  }
  if (r.payload.err == UINT32_C(2)) {
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
  }
  return r;
}

static uint32_t rt_scope_wait_all_count(rt_scope_t* s) {
  uint32_t slots_pending = 0;
  for (uint32_t i = 0; i < s->slot_len; i++) {
    if (s->slots[i].state == RT_SCOPE_SLOT_PENDING) slots_pending += 1;
  }
  return s->child_len + slots_pending;
}

static void rt_scope_reset_active(rt_scope_t* s) {
  s->child_len = 0;
  s->reg_len = 0;
  s->cancel_requested = 0;
}

static uint32_t rt_scope_cancel_all(ctx_t* ctx, rt_scope_t* s) {
  uint32_t canceled = 0;
  for (uint32_t i = s->reg_len; i > 0; i--) {
    canceled += rt_task_cancel(ctx, s->reg_task_ids[i - 1]);
  }
  s->cancel_requested = 1;
  return canceled;
}

static uint32_t rt_scope_join_drop_remaining_poll(ctx_t* ctx, rt_scope_t* s) {
  if (s->join_phase == 0) {
    s->join_phase = 1;
    s->join_slot_i = 0;
    s->join_child_i = 0;
  }

  if (s->join_phase == 1) {
    while (s->join_slot_i < s->slot_len) {
      uint32_t slot_id = s->join_slot_i;
      rt_scope_slot_t* slot = &s->slots[slot_id];
      if (slot->state != RT_SCOPE_SLOT_PENDING) {
        s->join_slot_i += 1;
        continue;
      }

      if (slot->kind == RT_TASK_OUT_KIND_BYTES) {
        bytes_t out = rt_bytes_empty(ctx);
        uint32_t task_id = slot->task_id;
        uint32_t done = rt_task_join_bytes_poll(ctx, task_id, &out);
        if (!done) return UINT32_C(0);
        rt_scope_check_slot_result_size(s, out);
        rt_bytes_drop(ctx, &out);
        rt_scope_unregister_task(s, task_id);
      } else if (slot->kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
        result_bytes_t out = (result_bytes_t){0};
        uint32_t task_id = slot->task_id;
        uint32_t done = rt_task_join_result_bytes_poll(ctx, task_id, &out);
        if (!done) return UINT32_C(0);
        if (out.tag) {
          rt_scope_check_slot_result_size(s, out.payload.ok);
          rt_bytes_drop(ctx, &out.payload.ok);
        }
        rt_scope_unregister_task(s, task_id);
      } else {
        rt_trap("scope slot kind invalid");
      }

      slot->state = RT_SCOPE_SLOT_CONSUMED;
      slot->task_id = 0;
      s->join_slot_i += 1;
    }
    s->join_phase = 2;
  }

  if (s->join_phase == 2) {
    while (s->join_child_i < s->child_len) {
      uint32_t task_id = s->child_task_ids[s->join_child_i];
      uint32_t kind = s->child_task_kinds[s->join_child_i];
      if (kind == RT_TASK_OUT_KIND_BYTES) {
        uint32_t done = rt_task_join_bytes_poll(ctx, task_id, NULL);
        if (!done) return UINT32_C(0);
      } else if (kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
        uint32_t done = rt_task_join_result_bytes_poll(ctx, task_id, NULL);
        if (!done) return UINT32_C(0);
      } else {
        rt_trap("scope child kind invalid");
      }
      s->join_child_i += 1;
    }
    s->join_phase = 0;
    s->join_slot_i = 0;
    s->join_child_i = 0;
    return UINT32_C(1);
  }

  rt_trap("scope join invalid state");
}

static void rt_scope_join_drop_remaining_block(ctx_t* ctx, rt_scope_t* s) {
  for (uint32_t slot_id = 0; slot_id < s->slot_len; slot_id++) {
    rt_scope_slot_t* slot = &s->slots[slot_id];
    if (slot->state != RT_SCOPE_SLOT_PENDING) continue;
    if (slot->kind == RT_TASK_OUT_KIND_BYTES) {
      uint32_t task_id = slot->task_id;
      bytes_t out = rt_task_join_bytes_block(ctx, task_id);
      rt_scope_check_slot_result_size(s, out);
      rt_bytes_drop(ctx, &out);
      rt_scope_unregister_task(s, task_id);
    } else if (slot->kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
      uint32_t task_id = slot->task_id;
      result_bytes_t out = rt_task_join_result_bytes_block(ctx, task_id);
      if (out.tag) {
        rt_scope_check_slot_result_size(s, out.payload.ok);
        rt_bytes_drop(ctx, &out.payload.ok);
      }
      rt_scope_unregister_task(s, task_id);
    } else {
      rt_trap("scope slot kind invalid");
    }
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
  }

  for (uint32_t i = 0; i < s->child_len; i++) {
    uint32_t task_id = s->child_task_ids[i];
    uint32_t kind = s->child_task_kinds[i];
    if (kind == RT_TASK_OUT_KIND_BYTES) {
      bytes_t out = rt_task_join_bytes_block(ctx, task_id);
      rt_bytes_drop(ctx, &out);
    } else if (kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
      result_bytes_t out = rt_task_join_result_bytes_block(ctx, task_id);
      if (out.tag) {
        rt_bytes_drop(ctx, &out.payload.ok);
      }
    } else {
      rt_trap("scope child kind invalid");
    }
  }
}

static void rt_scope_budget_check_exit(ctx_t* ctx, rt_scope_t* s) {
  uint64_t ticks = ctx->sched_now_ticks - s->snap_ticks;
  if (s->max_ticks != 0 && ticks > s->max_ticks) rt_trap("X07T_SCOPE_BUDGET_TICKS_EXCEEDED");

  uint64_t blocked = ctx->sched_stats.blocked_waits - s->snap_blocked_waits;
  if (s->max_blocked_waits != 0 && blocked > s->max_blocked_waits) {
    rt_trap("X07T_SCOPE_BUDGET_BLOCKED_WAITS_EXCEEDED");
  }

  uint64_t joins = ctx->sched_stats.join_calls - s->snap_join_polls;
  if (s->max_join_polls != 0 && joins > s->max_join_polls) rt_trap("X07T_SCOPE_BUDGET_JOIN_POLLS_EXCEEDED");
}

static uint32_t rt_scope_exit_poll(ctx_t* ctx, rt_scope_t* s) {
  uint32_t done = rt_scope_join_drop_remaining_poll(ctx, s);
  if (!done) return UINT32_C(0);
  rt_scope_budget_check_exit(ctx, s);
  rt_scope_drop(ctx, s);
  return UINT32_C(1);
}

static void rt_scope_exit_block(ctx_t* ctx, rt_scope_t* s) {
  rt_scope_join_drop_remaining_block(ctx, s);
  rt_scope_budget_check_exit(ctx, s);
  rt_scope_drop(ctx, s);
}

static void rt_scope_dispose_on_drop(ctx_t* ctx, rt_scope_t* s) {
  // Best-effort scope cleanup used by async task drop (task cancel / task complete):
  // - cancel unfinished children in deterministic order (reverse registration)
  // - drop any finished but unconsumed outputs (without blocking)
  // - free scope buffers
  if (s->reg_len != 0) {
    for (uint32_t i = s->reg_len; i > 0; i--) {
      uint32_t task_id = s->reg_task_ids[i - 1];
      if (task_id == 0) continue;
      rt_task_t* t = rt_task_ptr(ctx, task_id);
      if (!t->done) {
        (void)rt_task_cancel(ctx, task_id);
        continue;
      }
      if (t->out_taken || t->canceled) continue;
      if (t->out.kind == RT_TASK_OUT_KIND_BYTES) {
        (void)rt_task_join_bytes_poll(ctx, task_id, NULL);
      } else if (t->out.kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
        (void)rt_task_join_result_bytes_poll(ctx, task_id, NULL);
      } else {
        rt_trap("scope.drop invalid out kind");
      }
    }
  }
  rt_scope_drop(ctx, s);
}

static uint32_t rt_select_evt_new(
  ctx_t* ctx,
  uint64_t scope_key,
  uint32_t tag,
  uint32_t case_index,
  uint32_t src_id,
  bytes_t payload
) {
  // Reuse a free slot if possible.
  for (uint32_t i = 0; i < ctx->sched_select_evts_len; i++) {
    rt_select_evt_t* e = &ctx->sched_select_evts[i];
    if (e->alive) continue;
    memset(e, 0, sizeof(*e));
    e->alive = 1;
    e->tag = tag;
    e->case_index = case_index;
    e->src_id = src_id;
    e->scope_key = scope_key;
    e->payload = payload;
    return i + 1;
  }

  if (ctx->sched_select_evts_len == UINT32_MAX) rt_trap("select.evt.new overflow");
  uint32_t need = ctx->sched_select_evts_len + 1;
  rt_sched_select_evts_ensure_cap(ctx, need);
  uint32_t evt_id = need;
  rt_select_evt_t* e = &ctx->sched_select_evts[evt_id - 1];
  memset(e, 0, sizeof(*e));
  e->alive = 1;
  e->tag = tag;
  e->case_index = case_index;
  e->src_id = src_id;
  e->scope_key = scope_key;
  e->payload = payload;
  ctx->sched_select_evts_len = need;
  return evt_id;
}

static uint32_t rt_select_evt_tag(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  return e->tag;
}

static uint32_t rt_select_evt_case_index(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  return e->case_index;
}

static uint32_t rt_select_evt_src_id(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  return e->src_id;
}

static bytes_t rt_select_evt_take_bytes(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  if (e->taken) rt_trap("X07T_SELECT_EVT_ALREADY_TAKEN");
  e->taken = 1;
  bytes_t out = e->payload;
  e->payload = rt_bytes_empty(ctx);
  e->alive = 0;
  return out;
}

static void rt_select_evt_drop(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  if (e->taken) rt_trap("X07T_SELECT_EVT_ALREADY_TAKEN");
  rt_bytes_drop(ctx, &e->payload);
  e->payload = rt_bytes_empty(ctx);
  e->taken = 1;
  e->alive = 0;
}

static uint32_t rt_chan_bytes_new(ctx_t* ctx, uint32_t cap) {
  if (cap == 0) rt_trap("chan cap=0");
  rt_sched_chans_ensure_cap(ctx, ctx->sched_chans_len + 1);
  uint32_t chan_id = ctx->sched_chans_len + 1;
  rt_chan_bytes_t* c = &ctx->sched_chans[chan_id - 1];
  memset(c, 0, sizeof(*c));
  c->alive = 1;
  c->closed = 0;
  c->cap = cap;
  c->buf = (bytes_t*)rt_alloc(
    ctx,
    cap * (uint32_t)sizeof(bytes_t),
    (uint32_t)_Alignof(bytes_t)
  );
  c->head = 0;
  c->tail = 0;
  c->len = 0;
  ctx->sched_chans_len += 1;
  return chan_id;
}

static uint32_t rt_chan_bytes_send_poll(ctx_t* ctx, uint32_t chan_id, bytes_t msg) {
  ctx->sched_stats.chan_send_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("chan.send.poll from main");

  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) rt_trap("chan.send closed");

  if (c->len < c->cap) {
    c->buf[c->tail] = msg;
    c->tail = (c->tail + 1) % c->cap;
    c->len += 1;

    uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
    return UINT32_C(1);
  }

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_CHAN_SEND && me->wait_id == chan_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("chan.send while already waiting");
  me->wait_kind = RT_WAIT_CHAN_SEND;
  me->wait_id = chan_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_CHAN_SEND << 32) | chan_id);
  rt_wait_list_push(ctx, &c->send_wait_head, &c->send_wait_tail, cur);
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_recv_poll(ctx_t* ctx, uint32_t chan_id, bytes_t* out) {
  ctx->sched_stats.chan_recv_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("chan.recv.poll from main");

  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->len != 0) {
    bytes_t msg = c->buf[c->head];
    c->head = (c->head + 1) % c->cap;
    c->len -= 1;
    if (out) *out = msg;

    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
    return UINT32_C(1);
  }

  if (c->closed) {
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_CHAN_RECV && me->wait_id == chan_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("chan.recv while already waiting");
  me->wait_kind = RT_WAIT_CHAN_RECV;
  me->wait_id = chan_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_CHAN_RECV << 32) | chan_id);
  rt_wait_list_push(ctx, &c->recv_wait_head, &c->recv_wait_tail, cur);
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_send_block(ctx_t* ctx, uint32_t chan_id, bytes_t msg) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) rt_trap("chan.send closed");
  while (c->len >= c->cap) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    c = rt_chan_bytes_ptr(ctx, chan_id);
  }
  c->buf[c->tail] = msg;
  c->tail = (c->tail + 1) % c->cap;
  c->len += 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  return UINT32_C(1);
}

static uint32_t rt_chan_bytes_try_send_view(ctx_t* ctx, uint32_t chan_id, bytes_view_t msg) {
  ctx->sched_stats.chan_send_calls += 1;
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) return UINT32_C(2);
  if (c->len >= c->cap) return UINT32_C(0);

  c->buf[c->tail] = rt_view_to_bytes(ctx, msg);
  c->tail = (c->tail + 1) % c->cap;
  c->len += 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  return UINT32_C(1);
}

static result_bytes_t rt_chan_bytes_try_recv(ctx_t* ctx, uint32_t chan_id) {
  ctx->sched_stats.chan_recv_calls += 1;
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);

  if (c->len != 0) {
    bytes_t msg = c->buf[c->head];
    c->head = (c->head + 1) % c->cap;
    c->len -= 1;
    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
    return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = msg };
  }

  if (c->closed) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
}

static bytes_t rt_chan_bytes_recv_block(ctx_t* ctx, uint32_t chan_id) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  while (c->len == 0 && !c->closed) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    c = rt_chan_bytes_ptr(ctx, chan_id);
  }
  if (c->len == 0) return rt_bytes_empty(ctx);
  bytes_t msg = c->buf[c->head];
  c->head = (c->head + 1) % c->cap;
  c->len -= 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
  return msg;
}

static uint32_t rt_chan_bytes_close(ctx_t* ctx, uint32_t chan_id) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) return UINT32_C(0);
  c->closed = 1;
  for (;;) {
    uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
    if (w == 0) break;
    rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  }
  for (;;) {
    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w == 0) break;
    rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
  }
  return UINT32_C(1);
}

#if X07_ENABLE_STREAMING_FILE_IO
static uint32_t rt_io_reader_new_file(ctx_t* ctx, FILE* f, uint32_t pending_ticks) {
  if (!f) rt_trap("io.reader null file");
  rt_io_readers_ensure_cap(ctx, ctx->io_readers_len + 1);
  uint32_t reader_id = ctx->io_readers_len + 1;
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  memset(r, 0, sizeof(*r));
  r->alive = 1;
  r->kind = RT_IO_READER_KIND_FILE;
  r->eof = 0;
  r->pending_ticks = pending_ticks;
  r->f = f;
  r->bytes = rt_bytes_empty(ctx);
  r->pos = 0;
  ctx->io_readers_len += 1;
  return reader_id;
}
#else
static uint32_t rt_io_reader_new_file(ctx_t* ctx, void* f, uint32_t pending_ticks) {
  (void)ctx;
  (void)f;
  (void)pending_ticks;
  rt_trap("io.reader file disabled");
}
#endif

static uint32_t rt_io_reader_new_bytes(ctx_t* ctx, bytes_t b, uint32_t pending_ticks) {
  rt_io_readers_ensure_cap(ctx, ctx->io_readers_len + 1);
  uint32_t reader_id = ctx->io_readers_len + 1;
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  memset(r, 0, sizeof(*r));
  r->alive = 1;
  r->kind = RT_IO_READER_KIND_BYTES;
  r->eof = 0;
  r->pending_ticks = pending_ticks;
#if X07_ENABLE_STREAMING_FILE_IO
  r->f = NULL;
#endif
  r->bytes = b;
  r->pos = 0;
  ctx->io_readers_len += 1;
  return reader_id;
}

static uint32_t rt_io_read_poll(ctx_t* ctx, uint32_t reader_id, uint32_t max, bytes_t* out) {
  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader_id);
  if (max == 0 || r->eof) {
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep(ctx, ticks);
    return UINT32_C(0);
  }

  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    uint32_t n = (rem < max) ? rem : max;
    if (n) {
      r->pos = pos + n;
      bytes_t slice = (bytes_t){b.ptr + pos, n};
      if (out) *out = rt_bytes_clone(ctx, slice);
      return UINT32_C(1);
    }
    r->eof = 1;
    rt_bytes_drop(ctx, &r->bytes);
    r->bytes = rt_bytes_empty(ctx);
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

#if X07_ENABLE_STREAMING_FILE_IO
  if (r->kind != RT_IO_READER_KIND_FILE) rt_trap("io.read bad reader kind");
  if (!r->f) {
    r->eof = 1;
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  int c = fgetc(r->f);
  if (c == EOF) {
    fclose(r->f);
    r->f = NULL;
    r->eof = 1;
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  chunk.ptr[0] = (uint8_t)c;
  uint32_t got = 1;
  if (max > 1) {
    size_t n = fread(chunk.ptr + 1, 1, (size_t)(max - 1), r->f);
    if (n > (size_t)(UINT32_MAX - 1)) rt_trap("io.read too large");
    got += (uint32_t)n;
  }
  chunk.len = got;
  if (out) *out = chunk;
  return UINT32_C(1);
#else
  rt_trap("io.read bad reader kind");
#endif
}

static bytes_t rt_io_read_block(ctx_t* ctx, uint32_t reader_id, uint32_t max) {
  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader_id);
  if (max == 0 || r->eof) return rt_bytes_empty(ctx);

  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep_block(ctx, ticks);
  }

  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    uint32_t n = (rem < max) ? rem : max;
    if (n) {
      r->pos = pos + n;
      bytes_t slice = (bytes_t){b.ptr + pos, n};
      return rt_bytes_clone(ctx, slice);
    }
    r->eof = 1;
    rt_bytes_drop(ctx, &r->bytes);
    r->bytes = rt_bytes_empty(ctx);
    return rt_bytes_empty(ctx);
  }

#if X07_ENABLE_STREAMING_FILE_IO
  if (r->kind != RT_IO_READER_KIND_FILE) rt_trap("io.read bad reader kind");
  if (!r->f) {
    r->eof = 1;
    return rt_bytes_empty(ctx);
  }

  int c = fgetc(r->f);
  if (c == EOF) {
    fclose(r->f);
    r->f = NULL;
    r->eof = 1;
    return rt_bytes_empty(ctx);
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  chunk.ptr[0] = (uint8_t)c;
  uint32_t got = 1;
  if (max > 1) {
    size_t n = fread(chunk.ptr + 1, 1, (size_t)(max - 1), r->f);
    if (n > (size_t)(UINT32_MAX - 1)) rt_trap("io.read too large");
    got += (uint32_t)n;
  }
  chunk.len = got;
  return chunk;
#else
  rt_trap("io.read bad reader kind");
#endif
}

static bytes_t rt_iface_io_read_block(ctx_t* ctx, iface_t reader, uint32_t max) {
  if (max == 0) return rt_bytes_empty(ctx);
  if (reader.vtable == RT_IFACE_VTABLE_IO_READER) {
    return rt_io_read_block(ctx, reader.data, max);
  }
  if (reader.vtable < RT_IFACE_VTABLE_EXT_IO_READER_MIN || reader.vtable > RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    rt_trap("io.read bad iface vtable");
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, chunk.ptr, max);
  if (got == 0) {
    rt_bytes_drop(ctx, &chunk);
    return rt_bytes_empty(ctx);
  }
  chunk.len = got;
  return chunk;
}

static uint32_t rt_bufread_new(ctx_t* ctx, iface_t reader, uint32_t cap) {
  if (cap == 0) rt_trap("bufread cap=0");
  if (reader.vtable == RT_IFACE_VTABLE_IO_READER) {
    (void)rt_io_reader_ptr(ctx, reader.data);
  } else if (reader.vtable >= RT_IFACE_VTABLE_EXT_IO_READER_MIN && reader.vtable <= RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    // External IO reader: validated lazily on first read.
  } else {
    rt_trap("bufread.new bad iface vtable");
  }

  rt_bufreads_ensure_cap(ctx, ctx->bufreads_len + 1);
  uint32_t br_id = ctx->bufreads_len + 1;
  rt_bufread_t* br = &ctx->bufreads[br_id - 1];
  memset(br, 0, sizeof(*br));
  br->alive = 1;
  br->reader = reader;
  br->eof = 0;
  br->direct_bytes = 0;
  br->buf = rt_bytes_alloc(ctx, cap);
  br->start = 0;
  br->end = 0;
  ctx->bufreads_len += 1;
  return br_id;
}

static uint32_t rt_bufread_fill_poll(ctx_t* ctx, uint32_t br_id, bytes_view_t* out) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  if (br->start > br->end) rt_trap("bufread corrupt");
  uint32_t avail = br->end - br->start;
  if (avail != 0) {
    if (out) {
      if (br->direct_bytes) {
        iface_t reader = br->reader;
        if (reader.vtable != RT_IFACE_VTABLE_IO_READER) rt_trap("bufread corrupt");
        rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
        if (r->kind != RT_IO_READER_KIND_BYTES) rt_trap("bufread corrupt");
        *out = rt_bytes_subview(ctx, r->bytes, br->start, avail);
      } else {
        *out = rt_bytes_subview(ctx, br->buf, br->start, avail);
      }
    }
    return UINT32_C(1);
  }
  if (br->eof) {
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }

  iface_t reader = br->reader;
  if (reader.vtable != RT_IFACE_VTABLE_IO_READER) {
    uint32_t cap = br->buf.len;
    uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, br->buf.ptr, cap);
    br->direct_bytes = 0;
    br->start = 0;
    br->end = got;
    if (got == 0) {
      br->eof = 1;
      if (out) *out = rt_view_empty(ctx);
      return UINT32_C(1);
    }
    if (out) *out = rt_bytes_subview(ctx, br->buf, 0, got);
    return UINT32_C(1);
  }

  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
  if (r->eof) {
    br->eof = 1;
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }
  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep(ctx, ticks);
    return UINT32_C(0);
  }

  uint32_t cap = br->buf.len;
  uint32_t got = 0;
  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    got = (rem < cap) ? rem : cap;
    if (got) {
      br->direct_bytes = 1;
      br->start = pos;
      br->end = pos + got;
      r->pos = pos + got;
      if (out) *out = rt_bytes_subview(ctx, b, br->start, got);
      return UINT32_C(1);
    } else {
      r->eof = 1;
    }
  }
#if X07_ENABLE_STREAMING_FILE_IO
  else if (r->kind == RT_IO_READER_KIND_FILE) {
    if (!r->f) {
      r->eof = 1;
    } else {
      int c = fgetc(r->f);
      if (c == EOF) {
        fclose(r->f);
        r->f = NULL;
        r->eof = 1;
      } else {
        br->buf.ptr[0] = (uint8_t)c;
        got = 1;
        if (cap > 1) {
          size_t n = fread(br->buf.ptr + 1, 1, (size_t)(cap - 1), r->f);
          if (n > (size_t)(UINT32_MAX - 1)) rt_trap("bufread.fill too large");
          got += (uint32_t)n;
        }
      }
    }
  }
#endif
  else {
    rt_trap("bufread bad reader kind");
  }

  br->direct_bytes = 0;
  br->start = 0;
  br->end = got;
  if (got == 0) {
    br->eof = 1;
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }

  if (out) *out = rt_bytes_subview(ctx, br->buf, 0, got);
  return UINT32_C(1);
}

static bytes_view_t rt_bufread_fill_block(ctx_t* ctx, uint32_t br_id) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  for (;;) {
    if (br->start > br->end) rt_trap("bufread corrupt");
    uint32_t avail = br->end - br->start;
    if (avail != 0) {
      if (br->direct_bytes) {
        iface_t reader = br->reader;
        if (reader.vtable != RT_IFACE_VTABLE_IO_READER) rt_trap("bufread corrupt");
        rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
        if (r->kind != RT_IO_READER_KIND_BYTES) rt_trap("bufread corrupt");
        return rt_bytes_subview(ctx, r->bytes, br->start, avail);
      }
      return rt_bytes_subview(ctx, br->buf, br->start, avail);
    }
    if (br->eof) return rt_view_empty(ctx);

    iface_t reader = br->reader;
    if (reader.vtable != RT_IFACE_VTABLE_IO_READER) {
      uint32_t cap = br->buf.len;
      uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, br->buf.ptr, cap);
      br->direct_bytes = 0;
      br->start = 0;
      br->end = got;
      if (got == 0) {
        br->eof = 1;
        return rt_view_empty(ctx);
      }
      return rt_bytes_subview(ctx, br->buf, 0, got);
    }

    rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
    if (r->eof) {
      br->eof = 1;
      return rt_view_empty(ctx);
    }
    if (r->pending_ticks != 0) {
      uint32_t ticks = r->pending_ticks;
      r->pending_ticks = 0;
      rt_task_sleep_block(ctx, ticks);
      continue;
    }

    uint32_t cap = br->buf.len;
    uint32_t got = 0;
    if (r->kind == RT_IO_READER_KIND_BYTES) {
      bytes_t b = r->bytes;
      uint32_t pos = r->pos;
      if (pos > b.len) pos = b.len;
      uint32_t rem = b.len - pos;
      got = (rem < cap) ? rem : cap;
      if (got) {
        br->direct_bytes = 1;
        br->start = pos;
        br->end = pos + got;
        r->pos = pos + got;
        return rt_bytes_subview(ctx, b, br->start, got);
      } else {
        r->eof = 1;
      }
    }
#if X07_ENABLE_STREAMING_FILE_IO
    else if (r->kind == RT_IO_READER_KIND_FILE) {
      if (!r->f) {
        r->eof = 1;
      } else {
        int c = fgetc(r->f);
        if (c == EOF) {
          fclose(r->f);
          r->f = NULL;
          r->eof = 1;
        } else {
          br->buf.ptr[0] = (uint8_t)c;
          got = 1;
          if (cap > 1) {
            size_t n = fread(br->buf.ptr + 1, 1, (size_t)(cap - 1), r->f);
            if (n > (size_t)(UINT32_MAX - 1)) rt_trap("bufread.fill too large");
            got += (uint32_t)n;
          }
        }
      }
    }
#endif
    else {
      rt_trap("bufread bad reader kind");
    }

    br->direct_bytes = 0;
    br->start = 0;
    br->end = got;
    if (got == 0) {
      br->eof = 1;
      return rt_view_empty(ctx);
    }

    return rt_bytes_subview(ctx, br->buf, 0, got);
  }
}

static uint32_t rt_bufread_consume(ctx_t* ctx, uint32_t br_id, uint32_t n) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  if (br->start > br->end) rt_trap("bufread corrupt");
  uint32_t avail = br->end - br->start;
  if (n > avail) rt_trap("bufread.consume oob");
  br->start += n;
  if (br->start == br->end) {
    br->start = 0;
    br->end = 0;
    br->direct_bytes = 0;
  }
  return UINT32_C(0);
}

static uint32_t rt_fs_is_safe_rel_path(bytes_view_t path) {
  if (path.len == 0) return UINT32_C(0);
  if (path.ptr[0] == (uint8_t)'/') return UINT32_C(0);

  uint32_t seg_start = 0;
  for (uint32_t i = 0; i <= path.len; i++) {
    uint8_t b = (i == path.len) ? (uint8_t)'/' : path.ptr[i];
    if (i < path.len) {
      if (b == 0 || b == (uint8_t)'\\') return UINT32_C(0);
    }
    if (b == (uint8_t)'/') {
      uint32_t seg_len = i - seg_start;
      if (seg_len == 0) return UINT32_C(0);
      if (seg_len == 1 && path.ptr[seg_start] == (uint8_t)'.') return UINT32_C(0);
      if (seg_len == 2
          && path.ptr[seg_start] == (uint8_t)'.'
          && path.ptr[seg_start + 1] == (uint8_t)'.') return UINT32_C(0);
      if (seg_len >= 5 && memcmp(path.ptr + seg_start, ".x07_", 5) == 0) return UINT32_C(0);
      seg_start = i + 1;
    }
  }
  return UINT32_C(1);
}

static void rt_kv_init(ctx_t* ctx) {
  (void)ctx;
}

static void rt_hex_bytes(const uint8_t* bytes, uint32_t len, char* out) {
  static const char LUT[16] = "0123456789abcdef";
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = bytes[i];
    out[i * 2 + 0] = LUT[b >> 4];
    out[i * 2 + 1] = LUT[b & 0x0F];
  }
  out[len * 2] = 0;
}

static uint32_t rt_codec_read_u32_le(ctx_t* ctx, bytes_view_t buf, uint32_t offset) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes, buf.len);
#else
  (void)ctx;
#endif
  if (offset > buf.len || buf.len - offset < 4) rt_trap("codec.read_u32_le oob");
  return (uint32_t)buf.ptr[offset]
       | ((uint32_t)buf.ptr[offset + 1] << 8)
       | ((uint32_t)buf.ptr[offset + 2] << 16)
       | ((uint32_t)buf.ptr[offset + 3] << 24);
}

static bytes_t rt_codec_write_u32_le(ctx_t* ctx, uint32_t x) {
  bytes_t out = rt_bytes_alloc(ctx, 4);
  out.ptr[0] = (uint8_t)(x & UINT32_C(0xFF));
  out.ptr[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out.ptr[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out.ptr[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
  return out;
}

static bytes_t rt_fmt_u32_to_dec(ctx_t* ctx, uint32_t x) {
  uint8_t scratch[16];
  uint32_t n = 0;
  if (x == 0) {
    bytes_t out = rt_bytes_alloc(ctx, 1);
    out.ptr[0] = (uint8_t)'0';
    return out;
  }
  while (x > 0) {
    uint32_t digit = x % 10;
    x /= 10;
    scratch[n++] = (uint8_t)('0' + digit);
  }
  bytes_t out = rt_bytes_alloc(ctx, n);
  for (uint32_t i = 0; i < n; i++) {
    out.ptr[i] = scratch[n - 1 - i];
  }
  return out;
}

static bytes_t rt_fmt_s32_to_dec(ctx_t* ctx, uint32_t x) {
  if ((x & UINT32_C(0x80000000)) == 0) {
    return rt_fmt_u32_to_dec(ctx, x);
  }
  uint32_t mag = (~x) + UINT32_C(1);
  bytes_t digits = rt_fmt_u32_to_dec(ctx, mag);
  bytes_t out = rt_bytes_alloc(ctx, digits.len + 1);
  out.ptr[0] = (uint8_t)'-';
  memcpy(out.ptr + 1, digits.ptr, digits.len);
  rt_mem_on_memcpy(ctx, digits.len);
  return out;
}

static uint32_t rt_parse_u32_dec_slice(ctx_t* ctx, uint8_t* ptr, uint32_t len) {
  if (len == 0) rt_trap("parse.u32_dec empty");
  uint32_t acc = 0;
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = ptr[i];
    if (b < (uint8_t)'0' || b > (uint8_t)'9') rt_trap("parse.u32_dec non-digit");
    uint32_t digit = (uint32_t)(b - (uint8_t)'0');
    if (acc > (UINT32_MAX - digit) / 10) rt_trap("parse.u32_dec overflow");
    acc = acc * 10 + digit;
  }
  return acc;
}

static uint32_t rt_parse_u32_dec(ctx_t* ctx, bytes_view_t buf) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes, buf.len);
#endif
  return rt_parse_u32_dec_slice(ctx, buf.ptr, buf.len);
}

static uint32_t rt_parse_u32_dec_at(ctx_t* ctx, bytes_view_t buf, uint32_t offset) {
  if (offset > buf.len) rt_trap("parse.u32_dec_at oob");
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes + offset, buf.len - offset);
#endif
  return rt_parse_u32_dec_slice(ctx, buf.ptr + offset, buf.len - offset);
}

static uint32_t rt_prng_lcg_next_u32(uint32_t state) {
  return state * UINT32_C(1103515245) + UINT32_C(12345);
}

typedef struct {
  uint8_t* data;
  uint32_t len;
  uint32_t cap;
#ifdef X07_DEBUG_BORROW
  uint64_t dbg_aid;
#endif
} vec_u8_t;

static vec_u8_t rt_vec_u8_new(ctx_t* ctx, uint32_t cap) {
  vec_u8_t v;
  v.len = 0;
  v.cap = cap;
  v.data = (cap == 0) ? ctx->heap.mem : (uint8_t*)rt_alloc(ctx, cap, 1);
#ifdef X07_DEBUG_BORROW
  v.dbg_aid = (cap == 0) ? 0 : rt_dbg_alloc_register(ctx, v.data, cap);
#endif
  return v;
}

static void rt_vec_u8_drop(ctx_t* ctx, vec_u8_t* v) {
  if (!v) return;
  if (v->cap == 0) {
    v->data = ctx->heap.mem;
    v->len = 0;
    return;
  }
#ifdef X07_DEBUG_BORROW
  rt_dbg_alloc_kill(ctx, v->dbg_aid);
  v->dbg_aid = 0;
#endif
  rt_free(ctx, v->data, v->cap, 1);
  v->data = ctx->heap.mem;
  v->len = 0;
  v->cap = 0;
}

static uint32_t rt_vec_u8_len(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  return v.len;
}

static uint32_t rt_vec_u8_cap(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  return v.cap;
}

static vec_u8_t rt_vec_u8_clear(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  v.len = 0;
  return v;
}

static uint32_t rt_vec_u8_get(ctx_t* ctx, vec_u8_t v, uint32_t idx) {
  (void)ctx;
  if (idx >= v.len) rt_trap("vec_u8.get oob");
  return (uint32_t)v.data[idx];
}

static vec_u8_t rt_vec_u8_set(ctx_t* ctx, vec_u8_t v, uint32_t idx, uint32_t val) {
  (void)ctx;
  if (idx >= v.len) rt_trap("vec_u8.set oob");
  v.data[idx] = (uint8_t)(val & UINT32_C(0xFF));
  return v;
}

static vec_u8_t rt_vec_u8_push(ctx_t* ctx, vec_u8_t v, uint32_t val) {
  if (v.len == v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? (v.cap * 2) : 1;
    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }
  v.data[v.len++] = (uint8_t)(val & UINT32_C(0xFF));
  return v;
}

static vec_u8_t rt_vec_u8_reserve_exact(ctx_t* ctx, vec_u8_t v, uint32_t additional) {
  if (additional > UINT32_MAX - v.len) rt_trap("vec_u8.reserve_exact overflow");
  uint32_t need = v.len + additional;
  if (need <= v.cap) return v;

  uint8_t* old_data = v.cap ? v.data : NULL;
  uint32_t old_cap = v.cap;
  uint8_t* data = (uint8_t*)rt_alloc_realloc(
      ctx,
      old_data,
      old_cap,
      need,
      1
  );
  if (v.data && v.len) {
    memcpy(data, v.data, v.len);
    rt_mem_on_memcpy(ctx, v.len);
  }
#ifdef X07_DEBUG_BORROW
  rt_dbg_alloc_kill(ctx, v.dbg_aid);
  v.dbg_aid = rt_dbg_alloc_register(ctx, data, need);
#endif
  if (old_data && old_cap) {
    rt_free(ctx, old_data, old_cap, 1);
  }
  v.data = data;
  v.cap = need;
  return v;
}

static vec_u8_t rt_vec_u8_extend_zeroes(ctx_t* ctx, vec_u8_t v, uint32_t n) {
  if (n > UINT32_MAX - v.len) rt_trap("vec_u8.extend_zeroes overflow");
  uint32_t need = v.len + n;
  if (need > v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? v.cap : 1;
    while (new_cap < need) {
      if (new_cap > UINT32_MAX / 2) {
        new_cap = need;
        break;
      }
      new_cap *= 2;
    }

    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }

  if (n) {
    memset(v.data + v.len, 0, n);
  }
  v.len += n;
  return v;
}

static vec_u8_t rt_vec_u8_extend_bytes(ctx_t* ctx, vec_u8_t v, bytes_view_t b) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len);
#endif
  if (b.len > UINT32_MAX - v.len) rt_trap("vec_u8.extend_bytes overflow");
  uint32_t need = v.len + b.len;
  if (need > v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? v.cap : 1;
    while (new_cap < need) {
      if (new_cap > UINT32_MAX / 2) {
        new_cap = need;
        break;
      }
      new_cap *= 2;
    }

    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }

  if (b.len) {
    memcpy(v.data + v.len, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  v.len += b.len;
  return v;
}

static vec_u8_t rt_vec_u8_extend_bytes_range(
    ctx_t* ctx,
    vec_u8_t v,
    bytes_view_t b,
    uint32_t start,
    uint32_t len
) {
  if (start > b.len || b.len - start < len) rt_trap("vec_u8.extend_bytes_range oob");
  bytes_view_t sub;
  sub.ptr = b.ptr + start;
  sub.len = len;
#ifdef X07_DEBUG_BORROW
  sub.aid = b.aid;
  sub.bid = b.bid;
  if (UINT32_MAX - b.off_bytes < start) rt_trap("vec_u8.extend_bytes_range off overflow");
  sub.off_bytes = b.off_bytes + start;
#endif
  return rt_vec_u8_extend_bytes(ctx, v, sub);
}

static bytes_view_t rt_vec_u8_as_view(ctx_t* ctx, vec_u8_t v) {
  bytes_view_t out;
  out.len = v.len;
#ifdef X07_DEBUG_BORROW
  if (out.len == 0) {
    out.ptr = ctx->heap.mem;
    out.aid = 0;
    out.bid = 0;
    out.off_bytes = 0;
    return out;
  }
  out.ptr = v.data;
  out.aid = v.dbg_aid;
  out.off_bytes = 0;
  out.bid = rt_dbg_alloc_borrow_id(ctx, out.aid);
#else
  out.ptr = (out.len == 0) ? ctx->heap.mem : v.data;
#endif
  return out;
}

static bytes_t rt_vec_u8_into_bytes(ctx_t* ctx, vec_u8_t* v) {
  if (!v) return rt_bytes_empty(ctx);
  if (v->len == 0) {
    rt_vec_u8_drop(ctx, v);
    return rt_bytes_empty(ctx);
  }

  bytes_t out;
  out.ptr = v->data;
  out.len = v->len;

  v->data = ctx->heap.mem;
  v->len = 0;
  v->cap = 0;
#ifdef X07_DEBUG_BORROW
  v->dbg_aid = 0;
#endif
  return out;
}

#ifndef X07_JSON_JCS_ENABLED
#define X07_JSON_JCS_ENABLED 0
#endif

// --- X07_JSON_JCS_END

// --- X07_STREAM_XF_PLUGIN_END

struct rt_scratch_u8_fixed_s {
  uint32_t alive;
  vec_u8_t buf;
};

static void rt_scratch_u8_fixed_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->scratch_u8_fixed_cap) return;
  rt_scratch_u8_fixed_t* old_items = ctx->scratch_u8_fixed;
  uint32_t old_cap = ctx->scratch_u8_fixed_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_scratch_u8_fixed_t);
  uint32_t new_cap = ctx->scratch_u8_fixed_cap ? ctx->scratch_u8_fixed_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_scratch_u8_fixed_t* items = (rt_scratch_u8_fixed_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_scratch_u8_fixed_t),
    (uint32_t)_Alignof(rt_scratch_u8_fixed_t)
  );
  if (old_items && ctx->scratch_u8_fixed_len) {
    uint32_t bytes = ctx->scratch_u8_fixed_len * (uint32_t)sizeof(rt_scratch_u8_fixed_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_scratch_u8_fixed_t));
  }
  ctx->scratch_u8_fixed = items;
  ctx->scratch_u8_fixed_cap = new_cap;
}

static rt_scratch_u8_fixed_t* rt_scratch_u8_fixed_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->scratch_u8_fixed_len) rt_trap("scratch_u8_fixed invalid handle");
  rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[handle - 1];
  if (!s->alive) rt_trap("scratch_u8_fixed invalid handle");
  return s;
}

static uint32_t rt_scratch_u8_fixed_new(ctx_t* ctx, uint32_t cap) {
  // Reuse a free slot if possible.
  for (uint32_t i = 0; i < ctx->scratch_u8_fixed_len; i++) {
    rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[i];
    if (s->alive) continue;
    s->alive = 1;
    s->buf = rt_vec_u8_new(ctx, cap);
    return i + 1;
  }

  if (ctx->scratch_u8_fixed_len == UINT32_MAX) rt_trap("scratch_u8_fixed.new overflow");
  uint32_t need = ctx->scratch_u8_fixed_len + 1;
  rt_scratch_u8_fixed_ensure_cap(ctx, need);
  uint32_t handle = need;
  rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[handle - 1];
  s->alive = 1;
  s->buf = rt_vec_u8_new(ctx, cap);
  ctx->scratch_u8_fixed_len = need;
  return handle;
}

static uint32_t rt_scratch_u8_fixed_clear(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  s->buf.len = 0;
  return handle;
}

static uint32_t rt_scratch_u8_fixed_len(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  return s->buf.len;
}

static uint32_t rt_scratch_u8_fixed_cap(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  return s->buf.cap;
}

static bytes_view_t rt_scratch_u8_fixed_as_view(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  return rt_vec_u8_as_view(ctx, s->buf);
}

static result_i32_t rt_scratch_u8_fixed_try_write(ctx_t* ctx, uint32_t handle, bytes_view_t b) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len);
#endif
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  if (b.len > UINT32_MAX - s->buf.len) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(8) };
  }
  uint32_t need = s->buf.len + b.len;
  if (need > s->buf.cap) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(8) };
  }
  if (b.len) {
    memcpy(s->buf.data + s->buf.len, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  s->buf.len = need;
  return (result_i32_t){ .tag = UINT32_C(1), .payload.ok = b.len };
}

static uint32_t rt_scratch_u8_fixed_drop(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->scratch_u8_fixed_len) return UINT32_C(0);
  rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[handle - 1];
  if (!s->alive) return UINT32_C(0);
  rt_vec_u8_drop(ctx, &s->buf);
  s->alive = 0;
  return UINT32_C(1);
}

typedef struct {
  uint32_t cap;
  uint32_t len;
  uint32_t* keys;
  uint32_t* vals;
} map_u32_t;

static map_u32_t* rt_map_u32_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->map_u32_len) rt_trap("map_u32 invalid handle");
  map_u32_t* m = (map_u32_t*)ctx->map_u32_items[handle - 1];
  if (!m) rt_trap("map_u32 invalid handle");
  return m;
}

static uint32_t rt_is_pow2_u32(uint32_t x) {
  return (x != 0) && ((x & (x - 1)) == 0);
}

static uint32_t rt_map_u32_new(ctx_t* ctx, uint32_t cap) {
  if (!rt_is_pow2_u32(cap)) rt_trap("map_u32.new cap must be power of two");
  if (ctx->map_u32_len == ctx->map_u32_cap) {
    void** old_items = ctx->map_u32_items;
    uint32_t old_cap = ctx->map_u32_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->map_u32_cap ? (ctx->map_u32_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->map_u32_len) {
      uint32_t bytes = ctx->map_u32_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->map_u32_items = items;
    ctx->map_u32_cap = new_cap;
  }
  map_u32_t* m = (map_u32_t*)rt_alloc(ctx, (uint32_t)sizeof(map_u32_t), (uint32_t)_Alignof(map_u32_t));
  m->cap = cap;
  m->len = 0;
  m->keys = (uint32_t*)rt_alloc(ctx, cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  m->vals = (uint32_t*)rt_alloc(ctx, cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  memset(m->keys, 0xFF, cap * (uint32_t)sizeof(uint32_t));
  ctx->map_u32_items[ctx->map_u32_len++] = m;
  return ctx->map_u32_len;
}

static uint32_t rt_map_u32_len(ctx_t* ctx, uint32_t handle) {
  return rt_map_u32_ptr(ctx, handle)->len;
}

static uint32_t rt_map_u32_hash(uint32_t key) {
  return key * UINT32_C(2654435769);
}

static uint32_t rt_map_u32_get(ctx_t* ctx, uint32_t handle, uint32_t key, uint32_t default_) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.get key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) return m->vals[idx];
    if (slot == UINT32_C(0xFFFFFFFF)) return default_;
    idx = (idx + 1) & mask;
    if (idx == start) return default_;
  }
}

static uint32_t rt_map_u32_set(ctx_t* ctx, uint32_t handle, uint32_t key, uint32_t val) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.set key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) {
      m->vals[idx] = val;
      return UINT32_C(0);
    }
    if (slot == UINT32_C(0xFFFFFFFF)) {
      m->keys[idx] = key;
      m->vals[idx] = val;
      m->len += 1;
      return UINT32_C(1);
    }
    idx = (idx + 1) & mask;
    if (idx == start) rt_trap("map_u32 full");
  }
}

static uint32_t rt_map_u32_contains(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.contains key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) return UINT32_C(1);
    if (slot == UINT32_C(0xFFFFFFFF)) return UINT32_C(0);
    idx = (idx + 1) & mask;
    if (idx == start) return UINT32_C(0);
  }
}

static uint32_t rt_map_u32_remove(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.remove key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) break;
    if (slot == UINT32_C(0xFFFFFFFF)) return UINT32_C(0);
    idx = (idx + 1) & mask;
    if (idx == start) return UINT32_C(0);
  }

  m->keys[idx] = UINT32_C(0xFFFFFFFF);
  m->vals[idx] = 0;
  m->len -= 1;

  uint32_t j = (idx + 1) & mask;
  while (m->keys[j] != UINT32_C(0xFFFFFFFF)) {
    uint32_t k = m->keys[j];
    uint32_t v = m->vals[j];
    m->keys[j] = UINT32_C(0xFFFFFFFF);
    m->vals[j] = 0;
    m->len -= 1;
    (void)rt_map_u32_set(ctx, handle, k, v);
    j = (j + 1) & mask;
  }

  return UINT32_C(1);
}

static inline void rt_store_u32_le(uint8_t* out, uint32_t x) {
  out[0] = (uint8_t)(x & UINT32_C(0xFF));
  out[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static bytes_t rt_set_u32_dump_u32le(ctx_t* ctx, uint32_t handle) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (m->len == 0) return rt_bytes_empty(ctx);
  if (m->len > UINT32_MAX / UINT32_C(4)) rt_trap("set_u32.dump_u32le overflow");
  bytes_t out = rt_bytes_alloc(ctx, m->len * UINT32_C(4));
  uint32_t off = 0;
  for (uint32_t i = 0; i < m->cap; i++) {
    uint32_t k = m->keys[i];
    if (k == UINT32_C(0xFFFFFFFF)) continue;
    if (off > out.len - UINT32_C(4)) rt_trap("set_u32.dump_u32le len mismatch");
    rt_store_u32_le(out.ptr + off, k);
    off += UINT32_C(4);
  }
  if (off != out.len) rt_trap("set_u32.dump_u32le len mismatch");
  return out;
}

static bytes_t rt_map_u32_dump_kv_u32le_u32le(ctx_t* ctx, uint32_t handle) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (m->len == 0) return rt_bytes_empty(ctx);
  if (m->len > UINT32_MAX / UINT32_C(8)) rt_trap("map_u32.dump_kv_u32le_u32le overflow");
  bytes_t out = rt_bytes_alloc(ctx, m->len * UINT32_C(8));
  uint32_t off = 0;
  for (uint32_t i = 0; i < m->cap; i++) {
    uint32_t k = m->keys[i];
    if (k == UINT32_C(0xFFFFFFFF)) continue;
    if (off > out.len - UINT32_C(8)) rt_trap("map_u32.dump_kv_u32le_u32le len mismatch");
    rt_store_u32_le(out.ptr + off, k);
    rt_store_u32_le(out.ptr + off + UINT32_C(4), m->vals[i]);
    off += UINT32_C(8);
  }
  if (off != out.len) rt_trap("map_u32.dump_kv_u32le_u32le len mismatch");
  return out;
}

static uint32_t rt_hash_mix32(uint32_t h) {
  h ^= h >> 16;
  h *= UINT32_C(2246822507);
  h ^= h >> 13;
  h *= UINT32_C(3266489909);
  h ^= h >> 16;
  return h;
}

static uint32_t rt_hash_fnv1a32_view(ctx_t* ctx, bytes_view_t v) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, v.bid, v.off_bytes, v.len);
#else
  (void)ctx;
#endif
  uint32_t h = UINT32_C(2166136261);
  for (uint32_t i = 0; i < v.len; i++) {
    h ^= (uint32_t)v.ptr[i];
    h *= UINT32_C(16777619);
  }
  return h;
}

static int32_t rt_cmp_u32_to_i32(uint32_t x07_cmp) {
  if (x07_cmp == UINT32_MAX) return -1;
  if (x07_cmp == UINT32_C(1)) return 1;
  return 0;
}

typedef struct {
  uint32_t size;
  uint32_t align;
  void (*drop_in_place)(ctx_t* ctx, void* p);
  void (*clone_into)(ctx_t* ctx, void* dst, const void* src);
  uint32_t (*eq)(ctx_t* ctx, const void* a, const void* b);
  uint32_t (*hash32)(ctx_t* ctx, const void* a);
  int32_t (*cmp)(ctx_t* ctx, const void* a, const void* b);
} rt_tyops_v1;

static void rt_tyops_drop_nop(ctx_t* ctx, void* p) {
  (void)ctx;
  (void)p;
}

static void rt_tyops_clone_u32_into(ctx_t* ctx, void* dst, const void* src) {
  (void)ctx;
  *(uint32_t*)dst = *(const uint32_t*)src;
}

static uint32_t rt_tyops_eq_u32(ctx_t* ctx, const void* a, const void* b) {
  (void)ctx;
  return (*(const uint32_t*)a == *(const uint32_t*)b) ? UINT32_C(1) : UINT32_C(0);
}

static uint32_t rt_tyops_hash32_u32(ctx_t* ctx, const void* a) {
  (void)ctx;
  return rt_hash_mix32(*(const uint32_t*)a);
}

static int32_t rt_tyops_cmp_i32(ctx_t* ctx, const void* a, const void* b) {
  (void)ctx;
  int32_t aa = (int32_t)(*(const uint32_t*)a);
  int32_t bb = (int32_t)(*(const uint32_t*)b);
  if (aa < bb) return -1;
  if (aa > bb) return 1;
  return 0;
}

static int32_t rt_tyops_cmp_u32(ctx_t* ctx, const void* a, const void* b) {
  (void)ctx;
  uint32_t aa = *(const uint32_t*)a;
  uint32_t bb = *(const uint32_t*)b;
  if (aa < bb) return -1;
  if (aa > bb) return 1;
  return 0;
}

static void rt_tyops_drop_bytes(ctx_t* ctx, void* p) {
  rt_bytes_drop(ctx, (bytes_t*)p);
}

static void rt_tyops_clone_bytes_into(ctx_t* ctx, void* dst, const void* src) {
  bytes_t b = *(const bytes_t*)src;
  *(bytes_t*)dst = rt_bytes_clone(ctx, b);
}

static uint32_t rt_tyops_eq_bytes(ctx_t* ctx, const void* a, const void* b) {
  bytes_t ba = *(const bytes_t*)a;
  bytes_t bb = *(const bytes_t*)b;
  return rt_view_eq(ctx, rt_bytes_view(ctx, ba), rt_bytes_view(ctx, bb));
}

static uint32_t rt_tyops_hash32_bytes(ctx_t* ctx, const void* a) {
  bytes_t b = *(const bytes_t*)a;
  bytes_view_t v = rt_bytes_view(ctx, b);
  return rt_hash_mix32(rt_hash_fnv1a32_view(ctx, v));
}

static int32_t rt_tyops_cmp_bytes(ctx_t* ctx, const void* a, const void* b) {
  bytes_t ba = *(const bytes_t*)a;
  bytes_t bb = *(const bytes_t*)b;
  uint32_t r = rt_bytes_cmp_range(ctx, ba, 0, ba.len, bb, 0, bb.len);
  return rt_cmp_u32_to_i32(r);
}

static void rt_tyops_clone_bytes_view_into(ctx_t* ctx, void* dst, const void* src) {
  (void)ctx;
  *(bytes_view_t*)dst = *(const bytes_view_t*)src;
}

static uint32_t rt_tyops_eq_bytes_view(ctx_t* ctx, const void* a, const void* b) {
  bytes_view_t va = *(const bytes_view_t*)a;
  bytes_view_t vb = *(const bytes_view_t*)b;
  return rt_view_eq(ctx, va, vb);
}

static uint32_t rt_tyops_hash32_bytes_view(ctx_t* ctx, const void* a) {
  bytes_view_t v = *(const bytes_view_t*)a;
  return rt_hash_mix32(rt_hash_fnv1a32_view(ctx, v));
}

static int32_t rt_tyops_cmp_bytes_view(ctx_t* ctx, const void* a, const void* b) {
  bytes_view_t va = *(const bytes_view_t*)a;
  bytes_view_t vb = *(const bytes_view_t*)b;
  uint32_t r = rt_view_cmp_range(ctx, va, 0, va.len, vb, 0, vb.len);
  return rt_cmp_u32_to_i32(r);
}

static const rt_tyops_v1 RT_TYOPS_V1[5] = {
  {0},
  {
    .size = UINT32_C(4),
    .align = (uint32_t)_Alignof(uint32_t),
    .drop_in_place = rt_tyops_drop_nop,
    .clone_into = rt_tyops_clone_u32_into,
    .eq = rt_tyops_eq_u32,
    .hash32 = rt_tyops_hash32_u32,
    .cmp = rt_tyops_cmp_i32,
  },
  {
    .size = UINT32_C(4),
    .align = (uint32_t)_Alignof(uint32_t),
    .drop_in_place = rt_tyops_drop_nop,
    .clone_into = rt_tyops_clone_u32_into,
    .eq = rt_tyops_eq_u32,
    .hash32 = rt_tyops_hash32_u32,
    .cmp = rt_tyops_cmp_u32,
  },
  {
    .size = (uint32_t)sizeof(bytes_t),
    .align = (uint32_t)_Alignof(bytes_t),
    .drop_in_place = rt_tyops_drop_bytes,
    .clone_into = rt_tyops_clone_bytes_into,
    .eq = rt_tyops_eq_bytes,
    .hash32 = rt_tyops_hash32_bytes,
    .cmp = rt_tyops_cmp_bytes,
  },
  {
    .size = (uint32_t)sizeof(bytes_view_t),
    .align = (uint32_t)_Alignof(bytes_view_t),
    .drop_in_place = rt_tyops_drop_nop,
    .clone_into = rt_tyops_clone_bytes_view_into,
    .eq = rt_tyops_eq_bytes_view,
    .hash32 = rt_tyops_hash32_bytes_view,
    .cmp = rt_tyops_cmp_bytes_view,
  },
};

static const rt_tyops_v1* rt_tyops_v1_get(ctx_t* ctx, uint32_t ty_id) {
  (void)ctx;
  if (ty_id == UINT32_C(1)
      || ty_id == UINT32_C(2)
      || ty_id == UINT32_C(3)
      || ty_id == UINT32_C(4)) {
    return &RT_TYOPS_V1[ty_id];
  }
  rt_trap("value ty_id invalid");
}

typedef struct {
  uint32_t ty_id;
  uint32_t len;
  uint32_t cap;
  const rt_tyops_v1* ops;
  uint8_t* data;
} vec_value_t;

static vec_value_t* rt_vec_value_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->vec_value_len) rt_trap("vec_value invalid handle");
  vec_value_t* v = (vec_value_t*)ctx->vec_value_items[handle - 1];
  if (!v) rt_trap("vec_value invalid handle");
  return v;
}

static void rt_vec_value_grow_exact(ctx_t* ctx, vec_value_t* v, uint32_t new_cap) {
  if (new_cap <= v->cap) return;
  uint32_t esz = v->ops->size;
  uint32_t align = v->ops->align;
  if (esz != 0 && new_cap > UINT32_MAX / esz) rt_trap("vec_value cap overflow");
  uint32_t old_bytes_total = v->cap * esz;
  uint32_t new_bytes_total = new_cap * esz;

  uint8_t* old_data = v->cap ? v->data : NULL;
  uint8_t* data = (uint8_t*)rt_alloc_realloc(ctx, old_data, old_bytes_total, new_bytes_total, align);
  if (old_data && v->len) {
    uint32_t bytes = v->len * esz;
    memcpy(data, v->data, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_data && old_bytes_total) {
    rt_free(ctx, old_data, old_bytes_total, align);
  }
  v->data = data;
  v->cap = new_cap;
}

static void rt_vec_value_reserve_exact_in_place(ctx_t* ctx, vec_value_t* v, uint32_t additional) {
  if (additional > UINT32_MAX - v->len) rt_trap("vec_value.reserve_exact overflow");
  uint32_t need = v->len + additional;
  if (need <= v->cap) return;
  rt_vec_value_grow_exact(ctx, v, need);
}

static void rt_vec_value_grow_for_push(ctx_t* ctx, vec_value_t* v, uint32_t need) {
  uint32_t new_cap = v->cap ? v->cap : 1;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_vec_value_grow_exact(ctx, v, new_cap);
}

static uint32_t rt_vec_value_push_raw(ctx_t* ctx, vec_value_t* v, const void* x) {
  if (v->len == UINT32_MAX) rt_trap("vec_value.push overflow");
  uint32_t need = v->len + 1;
  if (need > v->cap) {
    rt_vec_value_grow_for_push(ctx, v, need);
  }
  uint32_t esz = v->ops->size;
  uint8_t* dst = v->data + (v->len * esz);
  memcpy(dst, x, esz);
  rt_mem_on_memcpy(ctx, esz);
  v->len = need;
  return UINT32_C(0);
}

static uint32_t rt_vec_value_with_capacity_v1(ctx_t* ctx, uint32_t ty_id, uint32_t cap) {
  const rt_tyops_v1* ops = rt_tyops_v1_get(ctx, ty_id);

  if (ctx->vec_value_len == ctx->vec_value_cap) {
    void** old_items = ctx->vec_value_items;
    uint32_t old_cap = ctx->vec_value_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->vec_value_cap ? (ctx->vec_value_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->vec_value_len) {
      uint32_t bytes = ctx->vec_value_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->vec_value_items = items;
    ctx->vec_value_cap = new_cap;
  }

  vec_value_t* v = (vec_value_t*)rt_alloc(
    ctx,
    (uint32_t)sizeof(vec_value_t),
    (uint32_t)_Alignof(vec_value_t)
  );
  v->ty_id = ty_id;
  v->ops = ops;
  v->len = 0;
  v->cap = cap;
  if (cap == 0) {
    v->data = ctx->heap.mem;
  } else {
    if (ops->size != 0 && cap > UINT32_MAX / ops->size) rt_trap("vec_value.with_capacity cap overflow");
    uint32_t bytes_total = cap * ops->size;
    v->data = (uint8_t*)rt_alloc(ctx, bytes_total, ops->align);
  }

  ctx->vec_value_items[ctx->vec_value_len++] = v;
  return ctx->vec_value_len;
}

static uint32_t rt_vec_value_len(ctx_t* ctx, uint32_t handle) {
  return rt_vec_value_ptr(ctx, handle)->len;
}

static uint32_t rt_vec_value_reserve_exact(ctx_t* ctx, uint32_t handle, uint32_t additional) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  rt_vec_value_reserve_exact_in_place(ctx, v, additional);
  return handle;
}

static uint32_t rt_vec_value_push_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (!(v->ty_id == UINT32_C(1) || v->ty_id == UINT32_C(2))) rt_trap("vec_value.push_i32_v1 ty mismatch");
  (void)rt_vec_value_push_raw(ctx, v, &x);
  return handle;
}

static uint32_t rt_vec_value_push_bytes_v1(ctx_t* ctx, uint32_t handle, bytes_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(3)) rt_trap("vec_value.push_bytes_v1 ty mismatch");
  (void)rt_vec_value_push_raw(ctx, v, &x);
  return handle;
}

static uint32_t rt_vec_value_push_bytes_view_v1(ctx_t* ctx, uint32_t handle, bytes_view_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(4)) rt_trap("vec_value.push_bytes_view_v1 ty mismatch");
  (void)rt_vec_value_push_raw(ctx, v, &x);
  return handle;
}

static uint32_t rt_vec_value_get_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, uint32_t default_) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (!(v->ty_id == UINT32_C(1) || v->ty_id == UINT32_C(2))) rt_trap("vec_value.get_i32_v1 ty mismatch");
  if (idx >= v->len) return default_;
  uint8_t* src = v->data + (idx * v->ops->size);
  return *(uint32_t*)src;
}

static bytes_t rt_vec_value_get_bytes_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, bytes_t default_) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(3)) rt_trap("vec_value.get_bytes_v1 ty mismatch");
  if (idx >= v->len) return default_;
  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  uint8_t* src = v->data + (idx * v->ops->size);
  v->ops->clone_into(ctx, &out, src);
  return out;
}

static bytes_view_t rt_vec_value_get_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t idx,
    bytes_view_t default_
) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(4)) rt_trap("vec_value.get_bytes_view_v1 ty mismatch");
  if (idx >= v->len) return default_;
  uint8_t* src = v->data + (idx * v->ops->size);
  return *(bytes_view_t*)src;
}

static uint32_t rt_vec_value_set_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, uint32_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (!(v->ty_id == UINT32_C(1) || v->ty_id == UINT32_C(2))) rt_trap("vec_value.set_i32_v1 ty mismatch");
  if (idx >= v->len) rt_trap("vec_value.set oob");
  uint8_t* dst = v->data + (idx * v->ops->size);
  *(uint32_t*)dst = x;
  return handle;
}

static uint32_t rt_vec_value_set_bytes_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, bytes_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(3)) rt_trap("vec_value.set_bytes_v1 ty mismatch");
  if (idx >= v->len) rt_trap("vec_value.set oob");
  uint8_t* dst = v->data + (idx * v->ops->size);
  v->ops->drop_in_place(ctx, dst);
  memcpy(dst, &x, v->ops->size);
  rt_mem_on_memcpy(ctx, v->ops->size);
  return handle;
}

static uint32_t rt_vec_value_set_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t idx,
    bytes_view_t x
) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(4)) rt_trap("vec_value.set_bytes_view_v1 ty mismatch");
  if (idx >= v->len) rt_trap("vec_value.set oob");
  uint8_t* dst = v->data + (idx * v->ops->size);
  *(bytes_view_t*)dst = x;
  return handle;
}

static uint32_t rt_vec_value_pop(ctx_t* ctx, uint32_t handle) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->len == 0) return handle;
  uint32_t idx = v->len - 1;
  uint8_t* dst = v->data + (idx * v->ops->size);
  v->ops->drop_in_place(ctx, dst);
  v->len = idx;
  return handle;
}

static uint32_t rt_vec_value_clear(ctx_t* ctx, uint32_t handle) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  uint32_t esz = v->ops->size;
  for (uint32_t i = 0; i < v->len; i++) {
    v->ops->drop_in_place(ctx, v->data + (i * esz));
  }
  v->len = 0;
  return handle;
}

typedef struct {
  uint32_t cap;
  uint32_t len;
  uint32_t k_ty_id;
  uint32_t v_ty_id;
  const rt_tyops_v1* k_ops;
  const rt_tyops_v1* v_ops;

  uint8_t* ctrl;    // 0 empty, 1 filled, 2 tombstone
  uint32_t* hashes; // cached hash32(key)
  uint8_t* keys;    // raw key bytes
  uint8_t* vals;    // raw val bytes
} map_value_t;

static map_value_t* rt_map_value_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->map_value_len) rt_trap("map_value invalid handle");
  map_value_t* m = (map_value_t*)ctx->map_value_items[handle - 1];
  if (!m) rt_trap("map_value invalid handle");
  return m;
}

static void rt_map_value_alloc_arrays(ctx_t* ctx, map_value_t* m, uint32_t cap) {
  if (cap == 0) rt_trap("map_value cap=0");
  if (!rt_is_pow2_u32(cap)) rt_trap("map_value cap must be power of two");

  m->cap = cap;
  m->len = 0;

  m->ctrl = (uint8_t*)rt_alloc(ctx, cap, 1);
  memset(m->ctrl, 0, cap);

  if (cap > UINT32_MAX / (uint32_t)sizeof(uint32_t)) rt_trap("map_value hashes overflow");
  uint32_t hashes_bytes = cap * (uint32_t)sizeof(uint32_t);
  m->hashes = (uint32_t*)rt_alloc(ctx, hashes_bytes, (uint32_t)_Alignof(uint32_t));
  memset(m->hashes, 0, hashes_bytes);

  uint32_t ksz = m->k_ops->size;
  uint32_t vsz = m->v_ops->size;
  if (ksz != 0 && cap > UINT32_MAX / ksz) rt_trap("map_value keys overflow");
  if (vsz != 0 && cap > UINT32_MAX / vsz) rt_trap("map_value vals overflow");
  m->keys = (uint8_t*)rt_alloc(ctx, cap * ksz, m->k_ops->align);
  m->vals = (uint8_t*)rt_alloc(ctx, cap * vsz, m->v_ops->align);
}

static void rt_map_value_free_arrays(ctx_t* ctx, map_value_t* m) {
  if (m->ctrl && m->cap) rt_free(ctx, m->ctrl, m->cap, 1);
  if (m->hashes && m->cap) {
    rt_free(
      ctx,
      m->hashes,
      m->cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (m->keys && m->cap) rt_free(ctx, m->keys, m->cap * m->k_ops->size, m->k_ops->align);
  if (m->vals && m->cap) rt_free(ctx, m->vals, m->cap * m->v_ops->size, m->v_ops->align);

  m->ctrl = NULL;
  m->hashes = NULL;
  m->keys = NULL;
  m->vals = NULL;
  m->cap = 0;
  m->len = 0;
}

static uint8_t* rt_map_value_key_ptr(map_value_t* m, uint32_t idx) {
  return m->keys + (idx * m->k_ops->size);
}

static uint8_t* rt_map_value_val_ptr(map_value_t* m, uint32_t idx) {
  return m->vals + (idx * m->v_ops->size);
}

static uint32_t rt_map_value_lookup_idx(
    ctx_t* ctx,
    map_value_t* m,
    const void* key,
    uint32_t hash,
    uint32_t* out_idx
) {
  if (m->cap == 0) return 0;
  uint32_t mask = m->cap - 1;
  uint32_t idx = hash & mask;
  uint32_t start = idx;
  for (;;) {
    uint8_t c = m->ctrl[idx];
    if (c == 0) return 0;
    if (c == 1) {
      if (m->hashes[idx] == hash) {
        uint8_t* kptr = rt_map_value_key_ptr(m, idx);
        if (m->k_ops->eq(ctx, kptr, key)) {
          if (out_idx) *out_idx = idx;
          return 1;
        }
      }
    }
    idx = (idx + 1) & mask;
    if (idx == start) return 0;
  }
}

static uint32_t rt_map_value_find_slot(
    ctx_t* ctx,
    map_value_t* m,
    const void* key,
    uint32_t hash,
    uint32_t* out_idx,
    uint32_t* out_found
) {
  if (m->cap == 0) rt_trap("map_value cap=0");
  uint32_t mask = m->cap - 1;
  uint32_t idx = hash & mask;
  uint32_t start = idx;
  uint32_t first_tomb = UINT32_MAX;
  for (;;) {
    uint8_t c = m->ctrl[idx];
    if (c == 0) {
      if (out_found) *out_found = 0;
      if (out_idx) *out_idx = (first_tomb == UINT32_MAX) ? idx : first_tomb;
      return 1;
    }
    if (c == 2) {
      if (first_tomb == UINT32_MAX) first_tomb = idx;
    } else if (c == 1) {
      if (m->hashes[idx] == hash) {
        uint8_t* kptr = rt_map_value_key_ptr(m, idx);
        if (m->k_ops->eq(ctx, kptr, key)) {
          if (out_found) *out_found = 1;
          if (out_idx) *out_idx = idx;
          return 1;
        }
      }
    } else {
      rt_trap("map_value ctrl corrupt");
    }

    idx = (idx + 1) & mask;
    if (idx == start) {
      if (first_tomb != UINT32_MAX) {
        if (out_found) *out_found = 0;
        if (out_idx) *out_idx = first_tomb;
        return 1;
      }
      return 0;
    }
  }
}

static void rt_map_value_rehash(ctx_t* ctx, map_value_t* m, uint32_t new_cap) {
  if (!rt_is_pow2_u32(new_cap)) rt_trap("map_value.rehash cap must be power of two");

  map_value_t tmp = *m;
  rt_map_value_alloc_arrays(ctx, &tmp, new_cap);

  uint32_t ksz = m->k_ops->size;
  uint32_t vsz = m->v_ops->size;

  for (uint32_t i = 0; i < m->cap; i++) {
    if (m->ctrl[i] != 1) continue;
    uint32_t hash = m->hashes[i];

    uint32_t mask = tmp.cap - 1;
    uint32_t idx = hash & mask;
    uint32_t start = idx;
    for (;;) {
      if (tmp.ctrl[idx] == 0) break;
      idx = (idx + 1) & mask;
      if (idx == start) rt_trap("map_value.rehash full");
    }

    tmp.ctrl[idx] = 1;
    tmp.hashes[idx] = hash;
    memcpy(rt_map_value_key_ptr(&tmp, idx), rt_map_value_key_ptr(m, i), ksz);
    rt_mem_on_memcpy(ctx, ksz);
    memcpy(rt_map_value_val_ptr(&tmp, idx), rt_map_value_val_ptr(m, i), vsz);
    rt_mem_on_memcpy(ctx, vsz);
    tmp.len += 1;
  }

  rt_map_value_free_arrays(ctx, m);
  *m = tmp;
}

static void rt_map_value_ensure_room_for_insert(ctx_t* ctx, map_value_t* m) {
  uint64_t lhs = ((uint64_t)m->len + 1ULL) * 10ULL;
  uint64_t rhs = (uint64_t)m->cap * 7ULL;
  if (lhs <= rhs) return;

  if (m->cap > UINT32_MAX / 2) rt_trap("map_value cap overflow");
  rt_map_value_rehash(ctx, m, m->cap * 2);
}

static uint32_t rt_map_value_set_raw(
    ctx_t* ctx,
    map_value_t* m,
    const void* key,
    const void* val,
    uint32_t* out_replaced
) {
  rt_map_value_ensure_room_for_insert(ctx, m);

  uint32_t hash = m->k_ops->hash32(ctx, key);
  uint32_t idx = 0;
  uint32_t found = 0;
  if (!rt_map_value_find_slot(ctx, m, key, hash, &idx, &found)) {
    if (m->cap > UINT32_MAX / 2) rt_trap("map_value cap overflow");
    rt_map_value_rehash(ctx, m, m->cap * 2);
    if (!rt_map_value_find_slot(ctx, m, key, hash, &idx, &found)) {
      rt_trap("map_value.set no slot");
    }
  }

  uint32_t ksz = m->k_ops->size;
  uint32_t vsz = m->v_ops->size;
  uint8_t* vptr = rt_map_value_val_ptr(m, idx);

  if (found) {
    if (out_replaced) *out_replaced = 1;
    m->v_ops->drop_in_place(ctx, vptr);
    memcpy(vptr, val, vsz);
    rt_mem_on_memcpy(ctx, vsz);
    return 0;
  }

  if (out_replaced) *out_replaced = 0;
  m->ctrl[idx] = 1;
  m->hashes[idx] = hash;
  memcpy(rt_map_value_key_ptr(m, idx), key, ksz);
  rt_mem_on_memcpy(ctx, ksz);
  memcpy(vptr, val, vsz);
  rt_mem_on_memcpy(ctx, vsz);
  m->len += 1;
  return 0;
}

static uint32_t rt_map_value_remove_raw(ctx_t* ctx, map_value_t* m, const void* key) {
  uint32_t hash = m->k_ops->hash32(ctx, key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, key, hash, &idx)) return 0;

  uint8_t* kptr = rt_map_value_key_ptr(m, idx);
  uint8_t* vptr = rt_map_value_val_ptr(m, idx);
  m->k_ops->drop_in_place(ctx, kptr);
  m->v_ops->drop_in_place(ctx, vptr);
  m->ctrl[idx] = 2;
  if (m->len == 0) rt_trap("map_value.remove len underflow");
  m->len -= 1;
  return 1;
}

static uint32_t rt_map_value_clear_in_place(ctx_t* ctx, map_value_t* m) {
  for (uint32_t i = 0; i < m->cap; i++) {
    if (m->ctrl[i] != 1) continue;
    m->k_ops->drop_in_place(ctx, rt_map_value_key_ptr(m, i));
    m->v_ops->drop_in_place(ctx, rt_map_value_val_ptr(m, i));
  }
  memset(m->ctrl, 0, m->cap);
  memset(m->hashes, 0, m->cap * (uint32_t)sizeof(uint32_t));
  m->len = 0;
  return 0;
}

static uint32_t rt_map_value_new_v1(ctx_t* ctx, uint32_t k_ty_id, uint32_t v_ty_id, uint32_t cap) {
  if (!rt_is_pow2_u32(cap)) rt_trap("map_value.new cap must be power of two");
  const rt_tyops_v1* k_ops = rt_tyops_v1_get(ctx, k_ty_id);
  const rt_tyops_v1* v_ops = rt_tyops_v1_get(ctx, v_ty_id);
  if (!k_ops->eq || !k_ops->hash32) rt_trap("map_value.new key ops missing");

  if (ctx->map_value_len == ctx->map_value_cap) {
    void** old_items = ctx->map_value_items;
    uint32_t old_cap = ctx->map_value_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->map_value_cap ? (ctx->map_value_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->map_value_len) {
      uint32_t bytes = ctx->map_value_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->map_value_items = items;
    ctx->map_value_cap = new_cap;
  }

  map_value_t* m = (map_value_t*)rt_alloc(
    ctx,
    (uint32_t)sizeof(map_value_t),
    (uint32_t)_Alignof(map_value_t)
  );
  memset(m, 0, sizeof(map_value_t));
  m->k_ty_id = k_ty_id;
  m->v_ty_id = v_ty_id;
  m->k_ops = k_ops;
  m->v_ops = v_ops;
  rt_map_value_alloc_arrays(ctx, m, cap);

  ctx->map_value_items[ctx->map_value_len++] = m;
  return ctx->map_value_len;
}

static uint32_t rt_map_value_len(ctx_t* ctx, uint32_t handle) {
  return rt_map_value_ptr(ctx, handle)->len;
}

static uint32_t rt_map_value_clear(ctx_t* ctx, uint32_t handle) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  (void)rt_map_value_clear_in_place(ctx, m);
  return handle;
}

static uint32_t rt_map_value_contains_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.contains_i32_v1 key ty mismatch");
  uint32_t hash = m->k_ops->hash32(ctx, &key);
  return rt_map_value_lookup_idx(ctx, m, &key, hash, NULL);
}

static uint32_t rt_map_value_contains_bytes_v1(ctx_t* ctx, uint32_t handle, bytes_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.contains_bytes_v1 key ty mismatch");
  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t ok = rt_map_value_lookup_idx(ctx, m, &key, hash, NULL);
  rt_bytes_drop(ctx, &key);
  return ok;
}

static uint32_t rt_map_value_contains_bytes_view_v1(ctx_t* ctx, uint32_t handle, bytes_view_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.contains_bytes_view_v1 key ty mismatch");
  uint32_t hash = m->k_ops->hash32(ctx, &key);
  return rt_map_value_lookup_idx(ctx, m, &key, hash, NULL);
}

static uint32_t rt_map_value_remove_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.remove_i32_v1 key ty mismatch");
  (void)rt_map_value_remove_raw(ctx, m, &key);
  return handle;
}

static uint32_t rt_map_value_remove_bytes_v1(ctx_t* ctx, uint32_t handle, bytes_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.remove_bytes_v1 key ty mismatch");
  (void)rt_map_value_remove_raw(ctx, m, &key);
  rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_remove_bytes_view_v1(ctx_t* ctx, uint32_t handle, bytes_view_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.remove_bytes_view_v1 key ty mismatch");
  (void)rt_map_value_remove_raw(ctx, m, &key);
  return handle;
}

static uint32_t rt_map_value_get_i32_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    uint32_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.get key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  uint8_t* vptr = rt_map_value_val_ptr(m, idx);
  return *(uint32_t*)vptr;
}

static bytes_t rt_map_value_get_i32_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;

  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  m->v_ops->clone_into(ctx, &out, rt_map_value_val_ptr(m, idx));
  return out;
}

static bytes_view_t rt_map_value_get_i32_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_view_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  return *(bytes_view_t*)rt_map_value_val_ptr(m, idx);
}

static uint32_t rt_map_value_get_bytes_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    uint32_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.get key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  uint32_t found = rt_map_value_lookup_idx(ctx, m, &key, hash, &idx);
  rt_bytes_drop(ctx, &key);
  if (!found) return default_;
  return *(uint32_t*)rt_map_value_val_ptr(m, idx);
}

static bytes_t rt_map_value_get_bytes_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  uint32_t found = rt_map_value_lookup_idx(ctx, m, &key, hash, &idx);
  rt_bytes_drop(ctx, &key);
  if (!found) return default_;

  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  m->v_ops->clone_into(ctx, &out, rt_map_value_val_ptr(m, idx));
  return out;
}

static bytes_view_t rt_map_value_get_bytes_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_view_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  uint32_t found = rt_map_value_lookup_idx(ctx, m, &key, hash, &idx);
  rt_bytes_drop(ctx, &key);
  if (!found) return default_;
  return *(bytes_view_t*)rt_map_value_val_ptr(m, idx);
}

static uint32_t rt_map_value_get_bytes_view_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    uint32_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.get key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  return *(uint32_t*)rt_map_value_val_ptr(m, idx);
}

static bytes_t rt_map_value_get_bytes_view_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;

  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  m->v_ops->clone_into(ctx, &out, rt_map_value_val_ptr(m, idx));
  return out;
}

static bytes_view_t rt_map_value_get_bytes_view_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_view_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  return *(bytes_view_t*)rt_map_value_val_ptr(m, idx);
}

static uint32_t rt_map_value_set_i32_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    uint32_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.set key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_i32_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_i32_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_view_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_bytes_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    uint32_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.set key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.set val ty mismatch");
  uint32_t replaced = 0;
  (void)rt_map_value_set_raw(ctx, m, &key, &val, &replaced);
  if (replaced) rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_set_bytes_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.set val ty mismatch");
  uint32_t replaced = 0;
  (void)rt_map_value_set_raw(ctx, m, &key, &val, &replaced);
  if (replaced) rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_set_bytes_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_view_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.set val ty mismatch");
  uint32_t replaced = 0;
  (void)rt_map_value_set_raw(ctx, m, &key, &val, &replaced);
  if (replaced) rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_set_bytes_view_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    uint32_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.set key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_bytes_view_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_bytes_view_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_view_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static void rt_ctx_cleanup(ctx_t* ctx) {
#ifdef X07_DEBUG_BORROW
  if (ctx->dbg_borrows && ctx->dbg_borrows_cap) {
    rt_free(
      ctx,
      ctx->dbg_borrows,
      ctx->dbg_borrows_cap * (uint32_t)sizeof(dbg_borrow_rec_t),
      (uint32_t)_Alignof(dbg_borrow_rec_t)
    );
  }
  if (ctx->dbg_allocs && ctx->dbg_allocs_cap) {
    rt_free(
      ctx,
      ctx->dbg_allocs,
      ctx->dbg_allocs_cap * (uint32_t)sizeof(dbg_alloc_rec_t),
      (uint32_t)_Alignof(dbg_alloc_rec_t)
    );
  }
  ctx->dbg_borrows = NULL;
  ctx->dbg_borrows_len = 0;
  ctx->dbg_borrows_cap = 0;
  ctx->dbg_allocs = NULL;
  ctx->dbg_allocs_len = 0;
  ctx->dbg_allocs_cap = 0;
#endif

  rt_os_process_cleanup(ctx);

  for (uint32_t i = 0; i < ctx->bufreads_len; i++) {
    rt_bufread_t* br = &ctx->bufreads[i];
    if (!br->alive) continue;
    rt_bytes_drop(ctx, &br->buf);
    br->buf = rt_bytes_empty(ctx);
    br->alive = 0;
  }
  if (ctx->bufreads && ctx->bufreads_cap) {
    rt_free(
      ctx,
      ctx->bufreads,
      ctx->bufreads_cap * (uint32_t)sizeof(rt_bufread_t),
      (uint32_t)_Alignof(rt_bufread_t)
    );
  }
  ctx->bufreads = NULL;
  ctx->bufreads_len = 0;
  ctx->bufreads_cap = 0;

  for (uint32_t i = 0; i < ctx->scratch_u8_fixed_len; i++) {
    rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[i];
    if (!s->alive) continue;
    rt_vec_u8_drop(ctx, &s->buf);
    s->alive = 0;
  }
  if (ctx->scratch_u8_fixed && ctx->scratch_u8_fixed_cap) {
    rt_free(
      ctx,
      ctx->scratch_u8_fixed,
      ctx->scratch_u8_fixed_cap * (uint32_t)sizeof(rt_scratch_u8_fixed_t),
      (uint32_t)_Alignof(rt_scratch_u8_fixed_t)
    );
  }
  ctx->scratch_u8_fixed = NULL;
  ctx->scratch_u8_fixed_len = 0;
  ctx->scratch_u8_fixed_cap = 0;

  for (uint32_t i = 0; i < ctx->io_readers_len; i++) {
    rt_io_reader_t* r = &ctx->io_readers[i];
    if (!r->alive) continue;
#if X07_ENABLE_STREAMING_FILE_IO
    if (r->kind == RT_IO_READER_KIND_FILE && r->f) {
      fclose(r->f);
      r->f = NULL;
    }
#endif
    if (r->kind == RT_IO_READER_KIND_BYTES) {
      rt_bytes_drop(ctx, &r->bytes);
      r->bytes = rt_bytes_empty(ctx);
    }
    r->alive = 0;
  }
  if (ctx->io_readers && ctx->io_readers_cap) {
    rt_free(
      ctx,
      ctx->io_readers,
      ctx->io_readers_cap * (uint32_t)sizeof(rt_io_reader_t),
      (uint32_t)_Alignof(rt_io_reader_t)
    );
  }
  ctx->io_readers = NULL;
  ctx->io_readers_len = 0;
  ctx->io_readers_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_chans_len; i++) {
    rt_chan_bytes_t* c = &ctx->sched_chans[i];
    if (!c->alive) continue;
    for (uint32_t j = 0; j < c->len; j++) {
      uint32_t idx = (c->head + j) % c->cap;
      rt_bytes_drop(ctx, &c->buf[idx]);
    }
    if (c->buf && c->cap) {
      rt_free(ctx, c->buf, c->cap * (uint32_t)sizeof(bytes_t), (uint32_t)_Alignof(bytes_t));
    }
    c->buf = NULL;
    c->alive = 0;
  }
  if (ctx->sched_chans && ctx->sched_chans_cap) {
    rt_free(
      ctx,
      ctx->sched_chans,
      ctx->sched_chans_cap * (uint32_t)sizeof(rt_chan_bytes_t),
      (uint32_t)_Alignof(rt_chan_bytes_t)
    );
  }
  ctx->sched_chans = NULL;
  ctx->sched_chans_len = 0;
  ctx->sched_chans_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_select_evts_len; i++) {
    rt_select_evt_t* e = &ctx->sched_select_evts[i];
    if (!e->alive) continue;
    rt_bytes_drop(ctx, &e->payload);
    e->payload = rt_bytes_empty(ctx);
    e->alive = 0;
    e->taken = 0;
  }
  if (ctx->sched_select_evts && ctx->sched_select_evts_cap) {
    rt_free(
      ctx,
      ctx->sched_select_evts,
      ctx->sched_select_evts_cap * (uint32_t)sizeof(rt_select_evt_t),
      (uint32_t)_Alignof(rt_select_evt_t)
    );
  }
  ctx->sched_select_evts = NULL;
  ctx->sched_select_evts_len = 0;
  ctx->sched_select_evts_cap = 0;

  if (ctx->sched_timers && ctx->sched_timers_cap) {
    rt_free(
      ctx,
      ctx->sched_timers,
      ctx->sched_timers_cap * (uint32_t)sizeof(rt_timer_ev_t),
      (uint32_t)_Alignof(rt_timer_ev_t)
    );
  }
  ctx->sched_timers = NULL;
  ctx->sched_timers_len = 0;
  ctx->sched_timers_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_tasks_len; i++) {
    rt_task_t* t = &ctx->sched_tasks[i];
    if (!t->alive) continue;
    if (!t->done) t->canceled = 1;
    t->done = 1;
    if (t->drop && t->fut) {
      t->drop(ctx, t->fut);
    }
    t->drop = NULL;
    t->fut = NULL;
    rt_task_out_drop(ctx, &t->out);
    t->out = rt_task_out_empty(ctx);
    t->out_taken = 0;
    t->alive = 0;
  }
  if (ctx->sched_tasks && ctx->sched_tasks_cap) {
    rt_free(
      ctx,
      ctx->sched_tasks,
      ctx->sched_tasks_cap * (uint32_t)sizeof(rt_task_t),
      (uint32_t)_Alignof(rt_task_t)
    );
  }
  ctx->sched_tasks = NULL;
  ctx->sched_tasks_len = 0;
  ctx->sched_tasks_cap = 0;
  ctx->sched_ready_head = 0;
  ctx->sched_ready_tail = 0;

  if (ctx->fs_latency_entries && ctx->fs_latency_len) {
    rt_free(
      ctx,
      ctx->fs_latency_entries,
      ctx->fs_latency_len * (uint32_t)sizeof(fs_latency_entry_t),
      (uint32_t)_Alignof(fs_latency_entry_t)
    );
  }
  ctx->fs_latency_entries = NULL;
  ctx->fs_latency_len = 0;
  rt_bytes_drop(ctx, &ctx->fs_latency_blob);
  ctx->fs_latency_blob = rt_bytes_empty(ctx);

#if X07_ENABLE_RR
  for (uint32_t i = 0; i < ctx->rr_handles_len; i++) {
    rr_handle_t* h = &ctx->rr_handles[i];
    if (!h->alive) continue;

    for (uint32_t j = 0; j < h->cassettes_len; j++) {
      rr_cassette_t* c = &h->cassettes[j];
      if (c->append_f) {
        fclose((FILE*)c->append_f);
        c->append_f = NULL;
      }
      if (c->entries && c->entries_cap) {
        rt_free(
          ctx,
          c->entries,
          c->entries_cap * (uint32_t)sizeof(rr_entry_desc_t),
          (uint32_t)_Alignof(rr_entry_desc_t)
        );
      }
      c->entries = NULL;
      c->entries_len = 0;
      c->entries_cap = 0;
      rt_bytes_drop(ctx, &c->blob);
      c->blob = rt_bytes_empty(ctx);
      rt_bytes_drop(ctx, &c->path);
      c->path = rt_bytes_empty(ctx);
      c->file_bytes = 0;
    }

    if (h->cassettes && h->cassettes_cap) {
      rt_free(
        ctx,
        h->cassettes,
        h->cassettes_cap * (uint32_t)sizeof(rr_cassette_t),
        (uint32_t)_Alignof(rr_cassette_t)
      );
    }
    h->cassettes = NULL;
    h->cassettes_len = 0;
    h->cassettes_cap = 0;
    h->alive = 0;
  }
  if (ctx->rr_handles && ctx->rr_handles_cap) {
    rt_free(
      ctx,
      ctx->rr_handles,
      ctx->rr_handles_cap * (uint32_t)sizeof(rr_handle_t),
      (uint32_t)_Alignof(rr_handle_t)
    );
  }
  ctx->rr_handles = NULL;
  ctx->rr_handles_len = 0;
  ctx->rr_handles_cap = 0;
  ctx->rr_current = 0;
#endif

  if (ctx->kv_latency_entries && ctx->kv_latency_len) {
    rt_free(
      ctx,
      ctx->kv_latency_entries,
      ctx->kv_latency_len * (uint32_t)sizeof(kv_latency_entry_t),
      (uint32_t)_Alignof(kv_latency_entry_t)
    );
  }
  ctx->kv_latency_entries = NULL;
  ctx->kv_latency_len = 0;
  rt_bytes_drop(ctx, &ctx->kv_latency_blob);
  ctx->kv_latency_blob = rt_bytes_empty(ctx);

#if X07_ENABLE_KV
  for (uint32_t i = 0; i < ctx->kv_len; i++) {
    rt_bytes_drop(ctx, &ctx->kv_items[i].key);
    rt_bytes_drop(ctx, &ctx->kv_items[i].val);
  }
  if (ctx->kv_items && ctx->kv_cap) {
    rt_free(
      ctx,
      ctx->kv_items,
      ctx->kv_cap * (uint32_t)sizeof(kv_entry_t),
      (uint32_t)_Alignof(kv_entry_t)
    );
  }
#endif
  ctx->kv_items = NULL;
  ctx->kv_len = 0;
  ctx->kv_cap = 0;

  for (uint32_t i = 0; i < ctx->map_u32_len; i++) {
    map_u32_t* m = (map_u32_t*)ctx->map_u32_items[i];
    if (!m) continue;
    if (m->keys && m->cap) {
      rt_free(ctx, m->keys, m->cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
    }
    if (m->vals && m->cap) {
      rt_free(ctx, m->vals, m->cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
    }
    rt_free(ctx, m, (uint32_t)sizeof(map_u32_t), (uint32_t)_Alignof(map_u32_t));
  }
  if (ctx->map_u32_items && ctx->map_u32_cap) {
    rt_free(
      ctx,
      ctx->map_u32_items,
      ctx->map_u32_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->map_u32_items = NULL;
  ctx->map_u32_len = 0;
  ctx->map_u32_cap = 0;

  for (uint32_t i = 0; i < ctx->vec_value_len; i++) {
    vec_value_t* v = (vec_value_t*)ctx->vec_value_items[i];
    if (!v) continue;
    uint32_t esz = v->ops->size;
    for (uint32_t j = 0; j < v->len; j++) {
      v->ops->drop_in_place(ctx, v->data + (j * esz));
    }
    if (v->data && v->cap) {
      if (esz != 0 && v->cap > UINT32_MAX / esz) rt_trap("vec_value.cleanup overflow");
      rt_free(ctx, v->data, v->cap * esz, v->ops->align);
    }
    rt_free(ctx, v, (uint32_t)sizeof(vec_value_t), (uint32_t)_Alignof(vec_value_t));
  }
  if (ctx->vec_value_items && ctx->vec_value_cap) {
    rt_free(
      ctx,
      ctx->vec_value_items,
      ctx->vec_value_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->vec_value_items = NULL;
  ctx->vec_value_len = 0;
  ctx->vec_value_cap = 0;

  for (uint32_t i = 0; i < ctx->map_value_len; i++) {
    map_value_t* m = (map_value_t*)ctx->map_value_items[i];
    if (!m) continue;
    (void)rt_map_value_clear_in_place(ctx, m);
    rt_map_value_free_arrays(ctx, m);
    rt_free(ctx, m, (uint32_t)sizeof(map_value_t), (uint32_t)_Alignof(map_value_t));
  }
  if (ctx->map_value_items && ctx->map_value_cap) {
    rt_free(
      ctx,
      ctx->map_value_items,
      ctx->map_value_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->map_value_items = NULL;
  ctx->map_value_len = 0;
  ctx->map_value_cap = 0;
}

void __CPROVER_assume(int);
void __CPROVER_assert(int, const char*);

static uint32_t user_example_main(ctx_t* ctx, bytes_view_t input, uint32_t p0);
static uint32_t user_std_codec_read_u32_le(ctx_t* ctx, bytes_view_t input, bytes_view_t p0, uint32_t p1);

#line 1 "example.x07.json"
static uint32_t user_example_main(ctx_t* ctx, bytes_view_t input, uint32_t p0) {
  {
    uint32_t t_i32_1 = UINT32_C(0);
    rt_fuel(ctx, 1);
    uint32_t t_i32_2 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_2 = p0;
    uint32_t t_i32_3 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_3 = p0;
    t_i32_1 = (t_i32_2 == t_i32_3);
    __CPROVER_assume(t_i32_1 != UINT32_C(0));
  }
  uint32_t out = UINT32_C(0);
  rt_fuel(ctx, 1);
  out = p0;
  {
    uint32_t t_i32_4 = UINT32_C(0);
    rt_fuel(ctx, 1);
    uint32_t t_i32_5 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_5 = out;
    uint32_t t_i32_6 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_6 = p0;
    t_i32_4 = (t_i32_5 == t_i32_6);
    __CPROVER_assert(t_i32_4 != UINT32_C(0), "X07T_CONTRACT_V1 {\"clause_id\":\"e0\",\"clause_index\":0,\"clause_ptr\":\"/decls/1/ensures/0/expr\",\"contract_kind\":\"ensures\",\"fn\":\"example.main\",\"witness\":[]}");
  }
  return out;
}

#line 1 "std/codec.x07.json"
static uint32_t user_std_codec_read_u32_le(ctx_t* ctx, bytes_view_t input, bytes_view_t p0, uint32_t p1) {
  uint32_t out = UINT32_C(0);
  rt_fuel(ctx, 1);
  bytes_view_t t_view_1 = rt_view_empty(ctx);
  rt_fuel(ctx, 1);
  t_view_1 = p0;
  uint32_t t_i32_2 = UINT32_C(0);
  rt_fuel(ctx, 1);
  t_i32_2 = p1;
  out = rt_codec_read_u32_le(ctx, t_view_1, t_i32_2);
  return out;
}

#line 1 "main.x07.json"
static bytes_t solve(ctx_t* ctx, bytes_view_t input) {
  bytes_t out = rt_bytes_empty(ctx);
  rt_fuel(ctx, 1);
  {
    uint32_t v_1 = UINT32_C(0);
    rt_fuel(ctx, 1);
    v_1 = UINT32_C(0);
    uint32_t v_2 = UINT32_C(0);
    rt_fuel(ctx, 1);
    bytes_view_t t_view_3 = rt_view_empty(ctx);
    rt_fuel(ctx, 1);
    t_view_3 = input;
    uint32_t t_i32_4 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_4 = v_1;
    v_2 = user_std_codec_read_u32_le(ctx, input, t_view_3, t_i32_4);
    rt_fuel(ctx, 1);
    uint32_t t_i32_5 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_5 = v_1;
    uint32_t t_i32_6 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_6 = UINT32_C(4);
    v_1 = t_i32_5 + t_i32_6;
    uint32_t v_7 = UINT32_C(0);
    rt_fuel(ctx, 1);
    uint32_t t_i32_8 = UINT32_C(0);
    rt_fuel(ctx, 1);
    t_i32_8 = v_2;
    v_7 = user_example_main(ctx, input, t_i32_8);
    rt_fuel(ctx, 1);
    out = rt_bytes_empty(ctx);
  }
  return out;
}


static uint8_t rt_dummy_heap_mem[1];
static int32_t rt_last_exit_code = 0;

int32_t x07_exit_code_v1(void) {
  return rt_last_exit_code;
}

bytes_t x07_solve_v2(
    uint8_t* arena_mem,
    uint32_t arena_cap,
    const uint8_t* input_ptr,
    uint32_t input_len
) {
  if (!arena_mem) {
    if (arena_cap != 0) rt_trap("arena_mem is NULL");
    arena_mem = rt_dummy_heap_mem;
  }

  ctx_t ctx;
  memset(&ctx, 0, sizeof(ctx));
  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);
  ctx.fuel = ctx.fuel_init;
  ctx.heap.mem = arena_mem;
  ctx.heap.cap = arena_cap;
  rt_heap_init(&ctx);
  rt_allocator_init(&ctx);
  rt_ext_ctx = &ctx;

#ifdef X07_DEBUG_BORROW
  rt_dbg_init(&ctx);
#endif

  rt_kv_init(&ctx);

  bytes_t input_bytes = rt_bytes_alloc(&ctx, input_len);
  if (input_len) {
    memcpy(input_bytes.ptr, input_ptr, input_len);
    rt_mem_on_memcpy(&ctx, input_len);
  }

  rt_mem_epoch_reset(&ctx);

  bytes_view_t input = rt_bytes_view(&ctx, input_bytes);
  bytes_t out = solve(&ctx, input);
  rt_last_exit_code = ctx.exit_code;
  rt_ext_ctx = NULL;

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_bytes_check(&ctx, out);
#endif

  rt_bytes_drop(&ctx, &input_bytes);

  return out;
}


static unsigned char x07_nondet_u8(void) {
  unsigned char value;
  return value;
}
static void x07_verify_harness(void) {
  uint8_t arena_mem[65536];
  ctx_t ctx;
  memset(&ctx, 0, sizeof(ctx));
  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);
  ctx.fuel = ctx.fuel_init;
  ctx.heap.mem = arena_mem;
  ctx.heap.cap = (uint32_t)sizeof(arena_mem);
  rt_heap_init(&ctx);
  rt_allocator_init(&ctx);
  rt_ext_ctx = &ctx;
  rt_kv_init(&ctx);
  uint8_t x07_verify_input[4];
  x07_verify_input[0] = x07_nondet_u8();
  x07_verify_input[1] = x07_nondet_u8();
  x07_verify_input[2] = x07_nondet_u8();
  x07_verify_input[3] = x07_nondet_u8();
  bytes_view_t input = (bytes_view_t){ .ptr = x07_verify_input, .len = UINT32_C(4) };
  bytes_t out = solve(&ctx, input);
  rt_bytes_drop(&ctx, &out);
  rt_ext_ctx = NULL;
}

