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

Concurrency notes:

- Pipes that contain concurrency stages (currently `std.stream.xf.par_map_stream_*_v1`) inject a `defasync` helper and rewrite the call site to `await` it.
- Concurrency pipes are rejected inside `defn` (allowed only in `solve` and `defasync`).
- Unordered `par_map_stream_*_v1` stages require `allow_nondet_v1=1` in `std.stream.cfg_v1`.

## Runtime pieces

- The emitted pipeline helper is ordinary x07AST lowered to C like any other function.
- JSON canonicalization (`std.stream.xf.json_canon_stream_v1`) uses a built-in C runtime canonicalizer (RFC 8785 / JCS) emitted by the backend.

Internal-only helpers / safety:

- Concurrency pipe helpers use internal-only builtins for slot bookkeeping; user code must not call them directly.
- Stream pipe helper function names are reserved (`<module_id>.__std_stream_pipe_v1_<hash>`); `x07c` rejects user-defined functions using this prefix.

## Filesystem streaming sink

Filesystem streaming sinks lower to OS fs builtins, which use the `x07_ext_fs_*` streaming write handle ABI:

- Header: `crates/x07c/include/x07_ext_fs_abi_v1.h`
- Native implementation: `crates/x07-ext-fs-native/src/lib.rs`
