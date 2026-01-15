# OS Process ABI v1 (`os.process.*`)

This document defines the **stable bytes encodings** used by the standalone-only subprocess builtins.

- Capture APIs (return `ResultBytesV1`):
  - `os.process.run_capture_v1(req: bytes, caps: bytes) -> bytes`
  - `os.process.spawn_capture_v1(req: bytes, caps: bytes) -> i32`
  - `os.process.try_join_capture_v1(handle: i32) -> option_bytes`
  - `os.process.join_capture_v1(handle: i32) -> bytes`
- Streaming spawn (uses the same `ProcReqV1` / `ProcCapsV1` layouts):
  - `os.process.spawn_piped_v1(req: bytes, caps: bytes) -> i32`

Agents should build requests and caps via:

- `stdlib/os/0.2.0/modules/std/os/process/req_v1.x07.json` (`std.os.process.req_v1.*`)
- `stdlib/os/0.2.0/modules/std/os/process/caps_v1.x07.json` (`std.os.process.caps_v1.pack`)

Callers must import `std.os.process.req_v1` and `std.os.process.caps_v1`.

Agents should decode results via:

- `stdlib/os/0.2.0/modules/std/os/process.x07.json` (`std.os.process.is_err`, `std.os.process.err_code`, `std.os.process.resp_*`)

## Conventions

- Little-endian for all multi-byte integers.
- `u8`: 1 byte
- `u32_le`: 4 bytes, unsigned, little-endian
- `bytes`: raw bytes (no implicit NUL terminator)

## Error Codes (`err_code: u32_le`)

- `1`: `POLICY_DENIED`
- `2`: `INVALID_REQUEST`
- `3`: `SPAWN_FAILED`
- `4`: `TIMEOUT`
- `5`: `OUTPUT_LIMIT`

## Result Bytes (`ResultBytesV1`) (capture APIs only)

All capture APIs return a single `bytes` value with this encoding.

### ERR

```
u8  tag = 0
u32_le err_code
u32_le flags (reserved; v1: 0)
```

### OK (`ProcRespV1`)

```
u8  tag = 1
u8  ver = 1
u32_le exit_code
u32_le flags
u32_le stdout_len
bytes  stdout
u32_le stderr_len
bytes  stderr
```

## Spawn Request (`ProcReqV1`)

```
u8  ver = 1
u8  flags (v1: MUST be 0)
u32_le argv_count
repeat argv_count:
  u32_le arg_len
  bytes  arg
u32_le env_count
repeat env_count:
  u32_le key_len
  bytes  key
  u32_le val_len
  bytes  val
u32_le cwd_len
bytes  cwd
u32_le stdin_len
bytes  stdin
```

Notes:

- `argv[0]` is the executable path. No shell expansion is performed.
- `cwd_len=0` means “inherit current working directory”.
- `cwd_len>0` sets the child working directory to `cwd`:
  - `run-os`: `cwd` is treated as a path string (no NUL bytes).
  - `run-os-sandboxed`: requires `policy.process.allow_cwd=true` and resolves `cwd` as a **safe relative path** joined against `policy.process.allow_cwd_roots` (first root that `chdir()` succeeds with is used).

## Resource Caps (`ProcCapsV1`)

```
u8  ver = 1
u32_le max_stdout_bytes
u32_le max_stderr_bytes
u32_le timeout_ms
u32_le max_total_bytes (0 = max_stdout_bytes + max_stderr_bytes)
```

## Piped API v1 (streaming)

These are standalone-only and operate on a handle returned by `os.process.spawn_piped_v1`.

- `os.process.stdout_read_v1(handle: i32, max: i32) -> bytes`
- `os.process.stderr_read_v1(handle: i32, max: i32) -> bytes`
- `os.process.stdin_write_v1(handle: i32, chunk: bytes) -> i32` (1 if accepted, 0 if closed)
- `os.process.stdin_close_v1(handle: i32) -> i32` (1 if closed, 0 if already closed)
- `os.process.try_wait_v1(handle: i32) -> i32` (1 if exited, 0 otherwise)
- `os.process.join_exit_v1(handle: i32) -> i32` (yield boundary; waits until exit)
- `os.process.take_exit_v1(handle: i32) -> i32`
  - returns `exit_code` on success
  - returns `-err_code` for subprocess errors (`err_code` table above)
  - traps if called before exit or called twice
