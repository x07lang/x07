Spec-ID: x07.spec.internal.stream-pipe@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Stream pipe (`std.stream.pipe_v1`)

Internal notes for the stream pipe special form (end-user docs live in `docs/language/stream-pipes.md`).

## Compiler pipeline

- Elaboration pass (helper injection + call-site rewrite): `crates/x07c/src/stream_pipe.rs`
- Entry point: `stream_pipe::elaborate_stream_pipes` is called from `crates/x07c/src/compile.rs`

The elaborator:

- validates descriptor shapes
- injects a per-pipeline helper function into the defining module
- rewrites the `std.stream.pipe_v1` call site to call the injected helper
- deduplicates injected helpers by a stable hash of the descriptor with `expr_v1` bodies elided

Brand typing notes (v1):

- The elaborator optionally computes an item brand along the pipeline (`typecheck_item_brands_v1`).
- It can insert `std.stream.xf.require_brand_v1` stages when `auto_require_brand_v1=1` (using `meta.brands_v1`).
- `verify_produced_brands_v1=1` inserts `require_brand_v1` validators for any source/stage that claims it produces branded items (runtime cost).

Concurrency notes:

- Pipes that contain concurrency stages (currently `std.stream.xf.par_map_stream_*_v1`) inject a `defasync` helper and rewrite the call site to `await` it.
- Concurrency pipes are rejected inside `defn` (allowed only in `solve` and `defasync`).
- Unordered `par_map_stream_*_v1` stages require `allow_nondet_v1=1` in `std.stream.cfg_v1`.

## Runtime pieces

- The emitted pipeline helper is ordinary x07AST lowered to C like any other function.
- Stream plugin stages (`std.stream.xf.plugin_v1`, and selected built-in `std.stream.xf.*_v1` stages) lower to internal-only plugin builtins:
  - `__internal.stream_xf.plugin_init_v1`
  - `__internal.stream_xf.plugin_step_v1`
  - `__internal.stream_xf.plugin_flush_v1`
- Plugin ids are resolved deterministically at compile time from the toolchain registry under `arch/stream/plugins/index.x07sp.json`, and the lowering emits `native_requires` entries for the owning native backend (currently `x07.stream.xf`).
- JSON canonicalization (`std.stream.xf.json_canon_stream_v1`) is implemented as a stream plugin (`xf.json_canon_stream_v1`) in the `x07.stream.xf` native backend, and uses the built-in C JSON JCS runtime section emitted by `crates/x07c/src/c_emit.rs`.
- Stream plugin emit APIs include `emit_view` (borrowed view output). The runtime may reject view emission with error `118` and plugins should fall back to `emit_alloc` + `emit_commit`.

Runtime errors:

- `70` = `E_BRAND_ITEM_TOO_LARGE` (require_brand item too large)
- `71` = `E_BRAND_VALIDATE_FAILED` (validator failed; payload includes brand id, validator id, code, item index)

Internal-only helpers / safety:

- Concurrency pipe helpers use internal-only builtins for slot bookkeeping; user code must not call them directly.
- Plugin-backed stream stages use internal-only plugin builtins; user code must not call them directly.
- Stream pipe helper function names are reserved (`<module_id>.__std_stream_pipe_v1_<hash>`); `x07c` rejects user-defined functions using this prefix.

## Filesystem streaming sink

Filesystem streaming sinks lower to OS fs builtins, which use the `x07_ext_fs_*` streaming write handle ABI:

- Header: `crates/x07c/include/x07_ext_fs_abi_v1.h`
- Native implementation: `crates/x07-ext-fs-native/src/lib.rs`
