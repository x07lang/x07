# Guide: Safe archives (tar/tgz/zip)

Archive extraction is a classic footgun: inputs are often untrusted and the unsafe default is to “just write files”.

X07’s canonical approach is **safe-by-default archive inspection/extraction** with:

- strict path policies (no traversal)
- explicit size/entry caps
- structured, machine-readable issues with rewrite hints

This guide is built around the external package `ext-archive-c` and repo-pinned profiles under `arch/archive/`.

## Canonical workflow

### 1) Pin a safe profile in `arch/`

Use the v1 safe profiles as your starting point:

- `arch/archive/profiles/tar_extract_safe_v1.archive.json`
- `arch/archive/profiles/tgz_extract_safe_v1.archive.json`
- `arch/archive/profiles/zip_extract_safe_v1.archive.json`

Lock the contract surface:

```bash
x07 arch check --write-lock
```

### 2) Use the hardened extract/list helpers

High-level entry point (built-in safe profiles):

- `std.archive.safe_extract_v1(out_root, bytes, profile_id)`

Arch-driven entry points (recommended for production repos that want pinned limits/policies):

- `std.archive.tar.extract_tree_from_arch_v1(profile_id, bytes)`
- `std.archive.tgz.extract_tree_from_arch_v1(profile_id, bytes)`
- `std.archive.zip.extract_tree_from_arch_v1(profile_id, bytes)`

OS streaming extract-to-fs (recommended for large archives):

- `std.archive.extract_os.extract_to_fs_path_from_arch_v1(profile_id, out_root, path, caps_read, caps_write)`

### 3) Treat failures as data (structured issues)

Doc-envelope entry points (`... -> bytes`) return an `std.doc` envelope. The `*_from_arch_v1` helpers return `result_bytes` (payload bytes on ok; deterministic error code on err).

- On success: `std.doc.payload_v1` is canonical JSON describing extracted/listed entries.
- On failure: `std.doc.error_msg_v1` is canonical JSON with schema `x07.archive.issue@0.1.0`, including:
  - `expected_form`
  - `rewrite_hint`

Runnable example:

- `docs/examples/agent-gate/archive-safe-extract/zip-hello/`
- `docs/examples/agent-gate/archive-extract-to-fs/zip-hello/`
- API reference: `docs/archive/archive-v1.md`

### Threat model summary

The minimal threats you must assume for archives sourced from users, the network, CI artifacts, or third-party vendors:

- **Path traversal**: `../...`, leading `/`, backslashes, non-UTF8 names.
- **Resource bombs**: too many entries, too-large files, high expansion ratios (especially `tgz`).
- **Malformed containers**: truncated headers, invalid central directory, checksum mismatches.

### Secure extraction checklist

Before writing any extracted data to disk:

- Enforce a strict relative path policy (reject absolute paths and `..` segments).
- Cap: `max_entries`, `max_total_out_bytes`, `max_file_bytes`.
- For `tgz`, cap: `max_inflate_out_bytes` and `max_inflate_ratio_x100`.
- Refuse links (symlinks/hardlinks) unless you have an explicit, audited policy.
- Treat archive metadata (modes/ownership/timestamps) as untrusted inputs.
- Write only under a dedicated output root and do not follow existing symlinks.

## Expert appendix

### Corpus regression suite

The repo includes a small security corpus under `tests/corpora/archive/` and a CI gate:

```bash
./scripts/ci/check_archive_corpus.sh
```

It covers:

- zip-slip path traversal attempts
- malformed ZIP central directory
- TAR directory entries + traversal attempts
- TGZ inflate ratio limits

### When to prefer `*_from_arch_v1`

Prefer `*_from_arch_v1` in production code when you want:

- limits and path policy treated as part of the repo contract (`arch/` + lockfiles)
- `x07 arch check` to enforce the allowed archive operations

Use `std.archive.safe_extract_v1` for:

- quick audit tooling
- small internal utilities
- cases where hardcoded v1 safe defaults are sufficient
