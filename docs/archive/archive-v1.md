# Archive v1 (safe list + extract) — External archive package

This document pins the **v1 contract** for safe archive parsing/extraction provided by `ext-archive-c`.

Package:

- `packages/ext/x07-ext-archive-c/0.1.6/`

Design goals:

- one canonical API per operation (list vs extract)
- safe-by-default path policies and caps
- deterministic outputs (canonical JSON)
- actionable, structured error diagnostics

---

## Formats and profiles

The v1 safe profiles are identified by `profile_id`:

- `tar_extract_safe_v1`
- `tgz_extract_safe_v1`
- `zip_extract_safe_v1`

Repo-pinned profiles live under:

- `arch/archive/profiles/`

Each profile enforces:

- a strict POSIX relative path policy (`posix_strict_v1`)
- explicit size limits (`max_entries`, `max_total_out_bytes`, `max_file_bytes`, …)
- for `tgz`, explicit gzip inflation caps (`max_inflate_out_bytes`, `max_inflate_ratio_x100`)

---

## Public API (module surface)

Top-level dispatch:

- `std.archive.safe_extract_v1(out_root: bytes_view, bytes: bytes_view, profile_id: bytes_view) -> bytes`

Format-specific modules:

- `std.archive.tar.list_v1(tar: bytes_view) -> bytes`
- `std.archive.tar.extract_tree_v1(tar: bytes_view) -> bytes`
- `std.archive.tar.extract_tree_from_arch_v1(profile_id: bytes_view, tar: bytes_view) -> result_bytes`

- `std.archive.tgz.list_v1(tgz: bytes_view) -> bytes`
- `std.archive.tgz.extract_tree_v1(tgz: bytes_view) -> bytes`
- `std.archive.tgz.extract_tree_from_arch_v1(profile_id: bytes_view, tgz: bytes_view) -> result_bytes`

- `std.archive.zip.list_v1(zip: bytes_view) -> bytes`
- `std.archive.zip.extract_tree_v1(zip: bytes_view) -> bytes`
- `std.archive.zip.extract_tree_from_arch_v1(profile_id: bytes_view, zip: bytes_view) -> result_bytes`

OS streaming extract-to-fs (for large archives):

- `std.archive.extract_os.safe_extract_to_fs_path_v1(out_root: bytes, path: bytes, caps_read: bytes, caps_write: bytes, profile_id: bytes_view) -> bytes`
- `std.archive.extract_os.extract_to_fs_path_from_arch_v1(profile_id: bytes_view, out_root: bytes, path: bytes, caps_read: bytes, caps_write: bytes) -> result_bytes`

Doc-envelope functions (`-> bytes`) return an `std.doc` envelope encoded as `bytes`. The `*_from_arch_v1` helpers return `result_bytes` (payload bytes on ok; deterministic error code on err).

---

## Success payloads

On success, the payload is canonical JSON:

- Doc envelope: `std.doc.payload_v1(doc)`
- `result_bytes`: `result_bytes.unwrap_or(res, bytes.alloc(0))`

Listing payload shape:

```json
{"entries":[{"path":"..."}]}
```

Extract-tree payload shape:

```json
{"entries":[{"path":"...","size":123,"data_b64":"..."}]}
```

For TAR/TGZ, entries also include `mode`.

Extract-to-fs payload shape:

```json
{"entries":[{"path":"...","size":123}]}
```

For TAR/TGZ extract-to-fs, entries also include `mode`.

---

## Errors and issue schema

On failure:

- `std.doc.error_code_v1` returns a deterministic error code.
- `std.doc.error_msg_v1` returns canonical JSON with schema `x07.archive.issue@0.1.0`.
- For `result_bytes`, use `result_bytes.err_code(res)` for the error code.

Issue fields:

- `schema_version`: always `x07.archive.issue@0.1.0`
- `kind`: stable error class (e.g. `path_policy`, `invalid_archive`, `inflate_ratio_limit`)
- `op`: the failing operation (e.g. `std.archive.zip.extract_tree_v1`)
- `profile_id`: the active profile (e.g. `zip_extract_safe_v1`)
- `path`: the offending entry path when applicable
- `code`: numeric error code (as a JSON number)
- `detail`: subcode for additional context
- `message`: short human-readable message
- `expected_form`: “expected shape” summary for this error class
- `rewrite_hint`: concrete action to fix or mitigate

---

## Arch integration

To make archive policies part of your repo’s explicit contract surface:

1. Add profiles under `arch/archive/profiles/`.
2. Reference them from `arch/archive/index.x07archive.json`.
3. Lock the surface:

```bash
x07 arch check --write-lock
```

This ensures:

- the profile IDs exist and are schema-valid
- the allowed archive symbols (`ops_allowed`) match exports in your vendored dependencies
