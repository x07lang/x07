# X07 filesystem v1 (std.os.fs via `ext-fs`)

This phase lands a **single canonical**, **agent-friendly** filesystem surface for X07 in OS worlds.

The deliverable is an external package (not a deterministic-world feature): `packages/ext/x07-ext-fs/0.1.0/`.

Normative contract: `docs/fs/fs-v1.md`.

## What’s implemented (in-tree)

### Pinned spec (v1)

- `docs/fs/fs-v1.md` — FS v1 contract (UTF‑8 path rules, `FsCapsV1`, `FsStatV1`, sorted text list outputs, deterministic error codes, sandbox policy knobs).

### Package + modules

- Package manifest: `packages/ext/x07-ext-fs/0.1.0/x07-package.json`
- Module root: `packages/ext/x07-ext-fs/0.1.0/modules`
- Modules:
  - `std.os.fs` — thin wrappers over the `os.fs.*_v1` builtins.
  - `std.os.fs.spec` — `FsCapsV1` pack/accessors + `FsStatV1` accessors + v1 error/flag constants.

### Native backend + staging

- Rust staticlib backend: `crates/x07-ext-fs-native/`
- ABI header (consumed by the C backend): `crates/x07c/include/x07_ext_fs_abi_v1.h`
- Build + stage into `deps/x07/`:

```bash
./scripts/build_ext_fs.sh
```

### OS-world integration (toolchain + runners)

FS v1 is exposed to X07 via standalone-only builtins:

- Compiler C emitter + type support: `crates/x07c/src/c_emit.rs` (`os.fs.*_v1`)
- Host runner auto-linking of staged native lib: `crates/x07-host-runner/src/lib.rs`
- OS runner policy parsing + env export: `crates/x07-os-runner/src/policy.rs`, `crates/x07-os-runner/src/main.rs`

To avoid module-id conflicts, `stdlib/os/0.2.0/` drops the old `std.os.fs` shim and `std.os.fs` is now owned by `ext-fs`.

### Sandbox policy (`run-os-sandboxed`)

- Policy schema: `schemas/run-os-policy.schema.json` (fs section)
- Smoke policy example: `tests/external_os/fs_policy_deny_smoke/run-os-policy.fs_policy_deny_smoke.json`

### Smoke verification

- Fixtures: `benchmarks/fixtures/os/fs-smoke-v1/`
- Programs:
  - `tests/external_os/fs_smoke_ok/src/main.x07.json`
  - `tests/external_os/fs_policy_deny_smoke/src/main.x07.json`
- Suites:
  - `benchmarks/smoke/fs-os-smoke.json`
  - `benchmarks/smoke/fs-os-sandboxed-policy-deny-smoke.json`

Run:

```bash
./scripts/ci/check_fs_smoke.sh
```

## Worlds

- `run-os`: enabled by default (policy allow flags default to permissive).
- `run-os-sandboxed`: enabled only via `run-os-policy` allowlists and caps clamping.
- Not available in deterministic `solve-*` worlds.

## Editing workflow (agent-friendly)

x07AST files (`*.x07.json`) should be edited via structured patches:

```bash
cargo run -p x07 -- ast apply-patch --in path.x07.json --patch patch.json --out path.x07.json --validate
```

See: `docs/dev/x07-ast.md`.

## Bundle assets (reference)

The original drop-in bundle for this phase is kept under:

- `docs/phases/assets/x07_ext_fs_v1_bundle.tar.gz`

## Future work (not in v1)

Potential v2 additions, kept out of v1 to preserve a small, auditable surface:

- Streaming file I/O via `std.io` (`open_read`, `open_write`) with explicit caps limits.
- A richer walk/list entry encoding (kind/size/mtime) instead of text lists.
- File copy, temp files/dirs, and cross-process file locks (run-os only; policy-gated).
