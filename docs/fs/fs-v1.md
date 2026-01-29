# FS v1 (run-os) — External Filesystem Package

This document pins the **v1 contract** for `ext-fs` (files + directories) for **agentic** X07 programs.

The design goals are:

- **Single canonical way**: one obvious function per operation; no aliases.
- **Agent-friendly**: paths are just UTF‑8 bytes using `/` separators; caps are a fixed binary struct.
- **Deterministic outputs where possible**: directory listings and walks are **sorted** and use a canonical newline format.
- **Hard sandbox control** in `run-os-sandboxed` via `run-os-policy` allowlists + caps clamping.

---

## Worlds

This package is available only in:

- `run-os`
- `run-os-sandboxed`

It must not be usable in deterministic `solve-*` worlds.

---

## Path encoding (v1)

All paths are passed as **bytes** that must decode as **UTF‑8**.

Canonical conventions:

- Use `/` as the separator on **all platforms** (Windows included).
- No NUL byte (`0x00`) allowed.
- No empty segments.
- `.` segments are ignored.
- `..` segments are **rejected** (path traversal prevention).
- In `run-os-sandboxed`, the effective path must be inside an allow‑root after normalization/canonicalization.

Hidden paths:

- A path is considered **hidden** if any segment starts with `.` (e.g., `.git`, `.env`, `dir/.file`).

---

## Caps encoding: `FsCapsV1`

`FsCapsV1` is a **fixed-length 24-byte** struct.

All integers are **u32_le**.

| Offset | Size | Field |
|---:|---:|---|
| 0  | 4 | `version` (must be `1`) |
| 4  | 4 | `max_read_bytes` |
| 8  | 4 | `max_write_bytes` |
| 12 | 4 | `max_entries` (for list/walk) |
| 16 | 4 | `max_depth` (for walk) |
| 20 | 4 | `flags` bitset |

Flags (u32):

- bit 0: `ALLOW_SYMLINKS`
- bit 1: `ALLOW_HIDDEN`
- bit 2: `CREATE_PARENTS` (mkdirs/write)
- bit 3: `OVERWRITE` (write)
- bit 4: `ATOMIC_WRITE` (write via temp + rename)

Effective limits in `run-os-sandboxed`:

- `effective_max_* = min(policy_max_*, caps_max_*)` for non‑zero caps values.
- If a caps max is `0`, it means “use policy/default”.

---

## Stat encoding: `FsStatV1`

`FsStatV1` is a fixed-length **16-byte** struct (u32_le fields).

| Offset | Size | Field |
|---:|---:|---|
| 0  | 4 | `version` (must be `1`) |
| 4  | 4 | `kind` |
| 8  | 4 | `size_bytes_u32` (0 for non-files) |
| 12 | 4 | `mtime_unix_s_u32` (0 if unknown) |

`kind` values:

- 0 = missing / does not exist
- 1 = file
- 2 = dir
- 3 = symlink
- 4 = other

---

## Text list outputs (directory + walk)

For deterministic agent usage, list outputs are **newline-separated UTF‑8**, **sorted ascending**, and **must end with a trailing `\n`**.

- `list_dir_sorted_text_v1` returns entry **names** (not full paths).
- `walk_glob_sorted_text_v1` returns **relative paths** from the walk root using `/` separators.

If the result set is empty, the output is the single byte `\n` (empty line list).

---

## Public API (package module surface)

Package: `packages/ext/x07-ext-fs/0.1.3/`

Modules:

- `packages/ext/x07-ext-fs/0.1.3/modules/std/os/fs.x07.json` (`std.os.fs`)
- `packages/ext/x07-ext-fs/0.1.3/modules/std/os/fs/spec.x07.json` (`std.os.fs.spec`)

### Caps helpers (`std.os.fs.spec`)

- `std.os.fs.spec.caps_default_v1() -> bytes`
- `std.os.fs.spec.caps_pack_v1(max_read_bytes, max_write_bytes, max_entries, max_depth, flags) -> bytes`
- `std.os.fs.spec.caps_*_v1(caps: bytes_view) -> i32` accessors

### Stat helpers (`std.os.fs.spec`)

- `std.os.fs.spec.stat_kind_v1(stat: bytes_view) -> i32`
- `std.os.fs.spec.stat_size_u32_v1(stat: bytes_view) -> i32`
- `std.os.fs.spec.stat_mtime_unix_s_u32_v1(stat: bytes_view) -> i32`

### FS operations (`std.os.fs`)

All operations take `caps: bytes` (FsCapsV1).

- `std.os.fs.read_all_v1(path: bytes, caps: bytes) -> result_bytes`
- `std.os.fs.write_all_v1(path: bytes, data: bytes, caps: bytes) -> result_i32`
- `std.os.fs.mkdirs_v1(path: bytes, caps: bytes) -> result_i32`
- `std.os.fs.remove_file_v1(path: bytes, caps: bytes) -> result_i32`
- `std.os.fs.remove_dir_all_v1(path: bytes, caps: bytes) -> result_i32`
- `std.os.fs.rename_v1(src: bytes, dst: bytes, caps: bytes) -> result_i32`
- `std.os.fs.list_dir_sorted_text_v1(path: bytes, caps: bytes) -> result_bytes`
- `std.os.fs.walk_glob_sorted_text_v1(root: bytes, glob: bytes, caps: bytes) -> result_bytes`
- `std.os.fs.stat_v1(path: bytes, caps: bytes) -> result_bytes` (Ok(payload = FsStatV1))

### Move-only usage note (important)

X07 `bytes` values are **move-only**. Since these APIs take `path: bytes` and `caps: bytes`, passing a variable consumes it.

Recommended pattern for reusable caps (and other bytes you need later):

- Keep a template: `caps0 = std.os.fs.spec.caps_default_v1()`
- Pass a fresh owned copy each time: `std.bytes.copy(caps0)`

Likewise, if you need to reuse a `data: bytes` buffer passed to `write_all_v1`, pass `std.bytes.copy(data)` instead.

---

## Builtins (runner/compiler integration)

The stdlib functions above call these builtins (standalone-only):

- `os.fs.read_all_v1(path, caps) -> result_bytes`
- `os.fs.write_all_v1(path, data, caps) -> result_i32`
- `os.fs.mkdirs_v1(path, caps) -> result_i32`
- `os.fs.remove_file_v1(path, caps) -> result_i32`
- `os.fs.remove_dir_all_v1(path, caps) -> result_i32`
- `os.fs.rename_v1(src, dst, caps) -> result_i32`
- `os.fs.list_dir_sorted_text_v1(path, caps) -> result_bytes`
- `os.fs.walk_glob_sorted_text_v1(root, glob, caps) -> result_bytes`
- `os.fs.stat_v1(path, caps) -> result_bytes`

In this repo, these builtins call the native backend:

- Rust staticlib: `crates/x07-ext-fs-native/`
- ABI header: `crates/x07c/include/x07_ext_fs_abi_v1.h`
- Build+stage script: `scripts/build_ext_fs.sh` (stages into `deps/x07/`)

---

## Error codes (deterministic, v1)

All FS builtins must return **deterministic** error codes (not raw OS errno).

Reserved v1 space: **60000–60999**.

| Code | Name |
|---:|---|
| 60001 | `FS_ERR_POLICY_DENY` |
| 60002 | `FS_ERR_DISABLED` |
| 60003 | `FS_ERR_BAD_PATH` |
| 60004 | `FS_ERR_BAD_CAPS` |
| 60010 | `FS_ERR_NOT_FOUND` |
| 60011 | `FS_ERR_ALREADY_EXISTS` |
| 60012 | `FS_ERR_NOT_DIR` |
| 60013 | `FS_ERR_IS_DIR` |
| 60014 | `FS_ERR_PERMISSION` |
| 60015 | `FS_ERR_IO` |
| 60016 | `FS_ERR_TOO_LARGE` |
| 60017 | `FS_ERR_TOO_MANY_ENTRIES` |
| 60018 | `FS_ERR_DEPTH_EXCEEDED` |
| 60019 | `FS_ERR_SYMLINK_DENIED` |
| 60020 | `FS_ERR_UNSUPPORTED` |

---

## Sandbox policy (`run-os-sandboxed`)

Policy schema: `schemas/run-os-policy.schema.json` (schema id: `x07.run-os-policy@0.1.0`).

FS policy fields:

- `fs.enabled`
- `fs.read_roots` (allowlist)
- `fs.write_roots` (allowlist)
- `fs.deny_hidden`
- `fs.allow_symlinks` (optional)
- `fs.allow_mkdir` / `fs.allow_remove` / `fs.allow_rename` (optional)
- `fs.allow_walk` / `fs.allow_glob` (optional)
- `fs.max_read_bytes` / `fs.max_write_bytes` / `fs.max_entries` / `fs.max_depth` (optional)

The runner must enforce:

- root allowlists,
- hidden deny,
- symlink rules,
- max limits,
- and operation allow flags.

---

## Smoke suites (this repo)

- `ci/suites/smoke/fs-os-smoke.json`
- `ci/suites/smoke/fs-os-sandboxed-policy-deny-smoke.json`

Run:

- `./scripts/ci/check_fs_smoke.sh`
