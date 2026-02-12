# Stdio v1 (run-os) — External Stdio Package

This document pins the **v1 contract** for `ext-stdio` (stdin/stdout/stderr) for long-lived **OS-world** X07 programs (including MCP stdio servers).

The design goals are:

- **Small, generic surface** (MCP framing stays in x07-mcp).
- **Agent-friendly caps**: a fixed binary struct with explicit size bounds.
- **Clear stdout/stderr separation**: protocol messages go to stdout; logs can go to stderr.

---

## Worlds

This package is available only in:

- `run-os`
- `run-os-sandboxed`

It must not be usable in deterministic `solve-*` worlds.

---

## Caps encoding: `StdioCapsV1`

`StdioCapsV1` is a **fixed-length 16-byte** struct.

All integers are **u32_le**.

| Offset | Size | Field |
|---:|---:|---|
| 0  | 4 | `version` (must be `1`) |
| 4  | 4 | `max_read_bytes` |
| 8  | 4 | `max_write_bytes` |
| 12 | 4 | `flags` (reserved; must be `0` in v1) |

Effective limits:

- `effective_max_* = min(policy_max_*, caps_max_*)` for non‑zero caps values.
- If a caps max is `0`, it means “use policy/default”.

---

## Public API (package module surface)

Package: `packages/ext/x07-ext-stdio/0.1.0/`

Modules:

- `packages/ext/x07-ext-stdio/0.1.0/modules/std/os/stdio.x07.json` (`std.os.stdio`)
- `packages/ext/x07-ext-stdio/0.1.0/modules/std/os/stdio/spec.x07.json` (`std.os.stdio.spec`)

### Caps helpers (`std.os.stdio.spec`)

- `std.os.stdio.spec.caps_default_v1() -> bytes`
- `std.os.stdio.spec.caps_pack_v1(max_read_bytes, max_write_bytes, flags) -> bytes`
- `std.os.stdio.spec.caps_*_v1(caps: bytes_view) -> i32` accessors

### Stdio operations (`std.os.stdio`)

All operations take `caps: bytes` (`StdioCapsV1`).

- `std.os.stdio.read_line_v1(caps: bytes) -> result_bytes`
  - Reads from stdin until `\n` or EOF.
  - Returns the line **without** the trailing LF.
  - If the input ends with `\r\n`, strips the trailing `\r`.
  - If EOF is encountered before reading any bytes, returns `err_eof_v1`.
  - If the line exceeds `max_read_bytes`, consumes through the next LF and returns `err_too_large_v1`.
- `std.os.stdio.write_stdout_v1(data: bytes, caps: bytes) -> result_i32`
- `std.os.stdio.write_stderr_v1(data: bytes, caps: bytes) -> result_i32`
  - Writes all bytes or returns an error.
  - Returns `ok(nbytes_written)` on success.
  - Errors with `err_too_large_v1` if `len(data) > max_write_bytes`.
- `std.os.stdio.flush_stdout_v1() -> result_i32`
- `std.os.stdio.flush_stderr_v1() -> result_i32`

### Move-only usage note

X07 `bytes` values are **move-only**. Since these APIs take `caps: bytes`, passing a variable consumes it.

Recommended pattern for reusable caps:

- Keep a template: `caps0 = std.os.stdio.spec.caps_default_v1()`
- Pass a fresh owned copy each time: `std.bytes.copy(caps0)`

---

## Builtins (runner/compiler integration)

The module functions above call these builtins (OS-world only):

- `os.stdio.read_line_v1(caps) -> result_bytes`
- `os.stdio.write_stdout_v1(data, caps) -> result_i32`
- `os.stdio.write_stderr_v1(data, caps) -> result_i32`
- `os.stdio.flush_stdout_v1() -> result_i32`
- `os.stdio.flush_stderr_v1() -> result_i32`

In this repo, these builtins call the native backend:

- Rust staticlib: `crates/x07-ext-stdio-native/`
- ABI header: `crates/x07c/include/x07_ext_stdio_abi_v1.h`
- Build+stage script: `scripts/build_ext_stdio.sh` (stages into `deps/x07/`)

---

## Error codes (deterministic, v1)

| Code | Name |
|---:|---|
| 60101 | `STDIO_ERR_DISABLED` |
| 60102 | `STDIO_ERR_POLICY_DENY` |
| 60104 | `STDIO_ERR_BAD_CAPS` |
| 60115 | `STDIO_ERR_IO` |
| 60116 | `STDIO_ERR_TOO_LARGE` |
| 60121 | `STDIO_ERR_EOF` |

