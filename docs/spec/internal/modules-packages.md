Spec-ID: x07.spec.internal.modules-packages@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Phase E: Modules, packages, and composable stdlib

Phase E adds a deterministic, file-backed module system and a minimal package+lockfile workflow for reproducible builds.

## Modules (language-level)

x07AST modules are JSON objects (`spec/x07ast.schema.json`):

- `imports`: array of module IDs (strings)
- `decls`: array of decl objects:
  - `{"kind":"defn", ...}`
  - `{"kind":"defasync", ...}` (Phase G2)
  - `{"kind":"export", ...}` (module files only)
- Entry programs additionally contain `solve` (an expression).

Function names are namespaced with dot-separated module IDs:

- Entry file functions start with `main.` (example: `main.helper`).
- Module `app.rle` functions start with `app.rle.` (example: `app.rle.encode`).

Only exported functions of imported modules may be called across module boundaries.

## Deterministic module resolution

Modules are resolved by explicit roots (no directory scanning):

- A module ID like `app.rle` resolves to `app/rle.x07.json` under a provided module root.
- Built-in stdlib modules are embedded in the compiler (see `stdlib/`).

### Standalone binding override (`std.world.*`)

In standalone-only worlds (`run-os`, `run-os-sandboxed`), `std.world.*` modules are resolved from `--module-root` only (no built-in fallback).
This enables the Phase H3 binding pattern: `std.fs` stays source-stable while the world selects the underlying adapter.

CLI example (standalone, without a project manifest):

- `cargo run -p x07c -- compile --program <program.x07.json> --world run-os --module-root stdlib/os/0.2.0/modules --module-root src`

## Project workflow (manifest + lockfile)

Project manifest (`x07.project@0.2.0`) is JSON:

- `world`: one of `solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`
- `entry`: entry program file (compiled as module `main`)
- `module_roots`: filesystem roots for non-builtin modules
- `link`: optional standalone link configuration:
  - `link.libs`: list of `-l` library names (example: `"m"`, `"ssl"`)
  - `link.search_paths`: list of `-L` search paths (project-relative)
  - `link.frameworks`: list of macOS frameworks
  - `link.static`: boolean for `-static`
- `dependencies`: path dependencies that contain `x07-package.json`
- `lockfile`: optional output path (default `x07.lock.json`)

Commands:

- Generate/update lockfile: `cargo run -p x07c -- lock --project <path/to/x07.json>`
- Build deterministic C output: `cargo run -p x07c -- build --project <path/to/x07.json> --out target/out.c`
- Compile+run native: `cargo run -p x07-host-runner -- --project <path/to/x07.json> --world solve-pure --input <case.bin>`

Note: `link.*` is only used by `x07-os-runner` when building standalone (run-os*) executables; deterministic runners ignore it.

Example project: `examples/phaseE/`.

## Packages (path deps)

Package manifest (`x07.package@0.1.0`) is JSON:

- `name`, `version`
- `module_root`: directory that contains module files
- `modules`: list of module IDs provided by the package
- `meta` (optional): package metadata used by standalone tooling (not part of deterministic evaluation worlds)
  - `meta.import_mode: "ffi"` marks a package that requires compiling/linking `ffi/*.c` sources.
  - `meta.ffi_libs: ["ssl","crypto",...]` is a list of `-l` library names needed by the package.

Standalone runner support:
- `x07-os-runner --auto-ffi` discovers `x07-package.json` adjacent to `--module-root` paths and automatically compiles `ffi/*.c` plus `meta.ffi_libs`.

Lockfiles pin:

- Each dependency package manifest SHA-256
- Each dependency module file SHA-256

## Built-in stdlib

The compiler ships a small, versioned stdlib package under `stdlib/std/0.1.1/`:

- Module IDs are declared in `stdlib/std/0.1.1/x07-package.json` and embedded in the compiler in `crates/x07c/src/builtin_modules.rs`.
  Keep these in sync with the on-disk module files and `stdlib.lock` (CI: `./scripts/ci/check_stdlib_lock.sh`).

- `std.vec`: wrappers around `vec_u8` (`with_capacity`, `push`, `extend_bytes`, `as_bytes`, ...)
- `std.slice`: `clamp`, `cmp_bytes`
- `std.bytes`: `reverse`, `concat`, `take`, `drop`, `copy`, `slice`, plus bytes_view helpers (`max_u8`, `sum_u8`, `count_u8`, `starts_with`, `ends_with`)
- `std.codec`: `read_u32_le`, `write_u32_le`
- `std.parse`: `u32_dec`, `u32_dec_at`, `i32_status_le`, `i32_status_le_at`
- `std.fmt`: `u32_to_dec`, `s32_to_dec`
- `std.prng`: `lcg_next_u32`, `x07rand32_v1_stream`
- `std.bit`: `popcount_u32`
- `std.text.ascii`: `normalize_lines`, `tokenize_words_lower`, `split_u8`, `split_lines_view`
- `std.text.slices`: X7SL v1 builder + accessors (`builder_*_v1`, `validate_v1`, `count_v1`, `start_v1`, `len_v1`, `view_at_v1`, `copy_at_v1`)
- `std.text.utf8`: `validate_or_empty`
- `std.test`: deterministic assertions + X7TEST status encoder (toolchain test harness)
- `std.regex-lite`: `find_literal`, `is_match_literal`, `count_matches_u32le` (letters + `.` + `*` subset)
- `std.json`: `canonicalize_small`, `extract_path_canon_or_err`
- `std.csv`: `sum_second_col_i32_status_le`, `sum_second_col_i32le_or_err`
- `std.map`: `word_freq_sorted_ascii`
- `std.set`: `unique_lines_sorted`
- `std.u32`: `read_le_at`, `write_le_at`, `push_le`, `pow2_ceil`
- `std.small_map`: sorted packed `bytes -> u32` map (`empty_bytes_u32`, `get_bytes_u32`, `put_bytes_u32`, ...)
- `std.small_set`: sorted packed bytes set (`empty_bytes`, `contains_bytes`, `insert_bytes`, ...)
- `std.hash`: deterministic hashing (`fnv1a32_*`, `mix32`)
- `std.hash_map`: deterministic u32 map wrappers (`with_capacity_u32`, `get_u32_or`, `set_u32`, ...)
- `std.hash_set`: deterministic u32 set + view-key set (`view_new`, `view_contains`, `view_insert`)
- `std.btree_map`: ordered u32->u32 map (`get_u32_u32_or`, `put_u32_u32`, ...)
- `std.btree_set`: ordered u32 set (`contains_u32`, `insert_u32`, ...)
- `std.deque_u32`: growable ring-buffer deque for u32 (`push_back`, `front_or`, `pop_front`, `emit_u32le`, ...)
- `std.heap_u32`: min-heap priority queue for u32 (`push`, `min_or`, `pop_min`, `emit_u32le`, ...)
- `std.bitset`: dense bitset (`set`, `test`, `intersection_count`)
- `std.slab`: handle-based pool for u32 values (`free_head_u32`, `alloc_u32`, `free_u32`, `get_u32`, `set_u32`)
- `std.lru_cache`: fixed-capacity LRU cache for u32 keys/values (`peek_u32_opt`, `touch_u32`, `put_u32`)
- `std.result`: `ok_i32_le`, `err0`, `chain_sum_csv_i32`
- `std.option`: `some_i32_le`, `none`
- `std.io`: `read` (Phase G2 streaming)
- `std.io.bufread`: `new`, `fill`, `consume` (Phase G2 buffering)
- `std.fs`: `read`, `read_async`, `read_task` (bind via `std.world.fs`), plus fixture-only ops `open_read`, `list_dir`, `list_dir_sorted`
- `std.world.fs`: fixture-backed adapter used by `std.fs.read` in deterministic worlds
- `std.rr`: `send_request`, `fetch`, `send` (solve-rr only)
- `std.kv`: `get`, `get_async`, `set`, `get_stream` (solve-kv only)
- `std.path`: `join`, `basename`, `extname`

## Standalone stdlib packages (Phase H3)

`stdlib/os/0.2.0/` provides OS-backed adapters and convenience aliases (resolved via `--module-root`):

- `std.world.fs` (OS-backed)
- `std.world.env`, `std.world.time`, `std.world.process`, `std.world.net`
- Pinned by `stdlib.os.lock` (checked by `./scripts/ci/check_stdlib_lock.sh`).
- `std.os.env`, `std.os.time`, `std.os.process`, `std.os.net` (aliases of `std.world.*`)

`std.os.fs` is provided by the external filesystem package `ext-fs` (`packages/ext/x07-ext-fs/0.1.0/`).

Note: `std.world.net.http_request` maps to `os.net.http_request`, which currently traps (reserved).
For OS-world HTTP today, use `ext-net` (`packages/ext/x07-ext-net/0.1.5/`) via `std.net.http.*` (see `docs/guides/networking.md` and `docs/net/net-v1.md`).
