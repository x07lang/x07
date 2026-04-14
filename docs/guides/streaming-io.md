# Guide: Streaming IO (large files)

Use streaming IO when you need to handle large inputs without loading whole blobs into memory.

This guide covers:

- streaming filesystem reads/writes (`ext-fs`)
- streaming archive extraction to disk (`ext-archive-c` + native `os.archive.*`)
- minimal function profiling (`X07_PROFILE=1`)

## Canonical workflow

### Filesystem streaming (`std.os.fs`)

For OS worlds (`run-os`, `run-os-sandboxed`), prefer:

- `std.os.fs.copy_file_v1(src, dst, caps_read, caps_write)` for simple file copies, or
- streaming handles for “pipe” style workflows:
  - `std.os.fs.stream_open_read_v1`
  - `std.os.fs.stream_read_some_v1`
  - `std.os.fs.stream_open_write_v1`
  - `std.os.fs.stream_write_all_v1`
  - `std.os.fs.stream_copy_to_end_v1`

API reference:

- `docs/fs/fs-v1.md`

### Streaming archive extraction to disk

Pure archive extraction (`std.archive.*.extract_tree_v1`) returns extracted file bytes (`data_b64`) and is intended for small inputs.

For large archives, extract directly to the filesystem:

- `std.archive.extract_os.safe_extract_to_fs_path_v1(out_root, path, caps_read, caps_write, profile_id)` (doc envelope `-> bytes`)
- `std.archive.extract_os.extract_to_fs_path_from_arch_v1(profile_id, out_root, path, caps_read, caps_write)` (`result_bytes`)

API reference:

- `docs/archive/archive-v1.md`
- `docs/guides/safe-archives.md`

### Profiling

Enable minimal function profiling with:

- `X07_PROFILE=1`

Reference:

- `docs/toolchain/profiling.md`

### Runnable example

- `docs/examples/agent-gate/archive-extract-to-fs/zip-hello/`

## Expert notes

- Prefer streaming handles when the input size is unknown or untrusted; `read_all_v1` is intended for small bounded inputs.
- Keep `chunk_bytes` reasonably sized (for example `65536`) to balance overhead and memory footprint.
- In `run-os-sandboxed`, file operations must be allowed by both the sandbox policy roots and the per-call caps; see `docs/fs/fs-v1.md`.
