# X07 Memory Model v2 (Rust-like ownership/borrowing)

Status: implemented; canonical reference: `docs/spec/x07-memory-management.md`.

This phase replaces the current “per-run arena + no free” model with a Rust-like model:

- `bytes` is **owned** (heap allocation, dropped automatically).
- `vec_u8` is **owned** and growable (dropped automatically).
- `bytes_view` is a **borrowed** view into `bytes`/`vec_u8`/runtime input buffers.
- `input` is a `bytes_view` (borrowed, read-only).
- `solve` returns **owned** `bytes`.

Non-goals:
- Backward compatibility with the previous `bytes` (non-owning slice) semantics.
- Multiple competing “ways” to do the same thing: one canonical API per concept.

This plan follows `docs/archive/x07-memory-management.md` and the placement rules in
`docs/dev/x07-policy.md` (ownership/borrowing/drop are kernel semantics).

---

## Canonical user model (single way)

Owned values (move-only):
- `bytes`: fixed-length owned byte buffer.
- `vec_u8`: growable owned byte buffer.

Borrowed values (copy):
- `bytes_view`: borrowed read-only slice into an owned buffer.

Canonical conversions:
- Borrow: `["bytes.view", b]` / `["bytes.subview", b, start, len]` / `["vec_u8.as_view", v]`.
- Copy a view into owned bytes: `["view.to_bytes", v]`.
- Finalize a builder without copying: `["std.vec.as_bytes", v]` (consumes `v`, wraps `vec_u8.into_bytes`).

Canonical rules:
- Passing `bytes` / `vec_u8` as a `bytes` / `vec_u8` argument **moves** (consumes) the value.
- Read-only APIs take `bytes_view` (the compiler may insert a borrow when passing `bytes`/`vec_u8`).
- While a `bytes_view` borrow is alive, the owner cannot be moved, freed, or reallocated.

---

## Milestones

### M1 — Deterministic heap allocator (kernel)

Goal: replace bump arena allocation with a deterministic allocator that supports `free`.

Repo changes:
- `crates/x07c/src/c_emit.rs`: replace `arena_t` allocator with a heap allocator:
  - `rt_alloc/rt_realloc/rt_free` operate within `X07_MEM_CAP`.
  - `mem_stats` remains deterministic and “epoch-resettable” (exclude input).
- `crates/x07-host-runner`: keep `-DX07_MEM_CAP=...` and `--debug-borrow-checks` wiring.

Exit criteria:
- Existing suites still run deterministically.
- `mem_stats` leak gate remains meaningful (allocations after epoch reset must be freed/dropped).

### M2 — ABI v2 + runtime drop helpers (kernel)

Goal: define owned layouts and drop behavior.

Repo changes:
- `docs/spec/abi/`: introduce ABI v2 and update headers under `crates/x07c/include/`.
- Runtime adds:
  - `rt_bytes_drop(&bytes)` and `rt_vec_u8_drop(&vec_u8)`.
  - `rt_view_to_bytes(bytes_view)` (copy).
  - `rt_vec_u8_into_bytes(vec_u8*)` (move buffer out, no memcpy).

Exit criteria:
- `bytes`/`vec_u8` allocations are freed via drops; no reliance on “reset all memory” to pass leak gates.

### M3 — Compiler: ownership/moves + drop glue (release)

Goal: RAII-like behavior without runtime borrow state in release.

Repo changes:
- `crates/x07c`: enforce move tracking for all owned types.
- Emit drop glue at end of scopes (`begin`, `if` branches, loop bodies, functions).
- Ensure early returns drop in-scope owned values.

Exit criteria:
- Use-after-move is a compile error.
- Leak-free property is enforced by drops, not by “end-of-run cleanup”.

### M4 — Compiler: lexical borrow checker + debug borrow runtime

Goal: Rust-like borrowing rules.

Repo changes:
- `crates/x07c`: lexical borrow checker:
  - many shared borrows or one mutable borrow (mutable views are optional later).
  - forbid vec growth/realloc while borrowed.
  - forbid moving an owner while borrowed.
- Extend `X07_DEBUG_BORROW` runtime to a full deterministic borrow table (acquire/release/check),
  as outlined in `docs/archive/x07-memory-management.md`.

Exit criteria:
- Borrow violations are compile-time errors in release.
- Debug mode detects use-after-free / borrow conflicts deterministically.

### M5 — Stdlib + x07import regeneration + docs alignment

Goal: make the new model the only model.

Repo changes:
- `crates/x07c`: remove legacy box helpers and make `input` a `bytes_view`.
- `stdlib/std/0.1.1/modules/**`: update APIs to take `bytes_view` for read-only inputs and use
  `view.to_bytes` / `std.vec.as_bytes` for ownership boundaries.
- `import_sources/**` + `crates/x07import-core`: update imported module sources/types and regenerate.
- Update:
  - `crates/x07c/src/guide.rs`
  - `docs/spec/*` (language guide, memory management, ABI)
  - repo READMEs (`README.md`, `docs/README.md`) and meta (`AGENTS.md`, `CLAUDE.md`)

Exit criteria:
- `./scripts/ci/check_x07import_generated.sh` passes.
- `./scripts/ci/check_x07import_diagnostics_sync.sh` passes (if diagnostics changed).
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, and canary scripts pass.

### M6 — Scoped regions (optional, kernel + compiler)

Goal: allow bounded early bulk-frees for temporary work without exposing `free`.

Design constraints:
- Deterministic and resource-bounded (no ambient allocation behavior).
- No cross-region borrows in safe code; regions must be lexical.

Repo changes:
- Add a `region` handle type (or compiler-only notion) and a small set of primitives:
  - `region.new` / `region.drop` (kernel) or a structured form `with_region` (compiler feature + kernel hook).
- Compiler enforces: values allocated in an inner region cannot outlive it; views cannot outlive owners/regions.

Exit criteria:
- Leak gates still enforce `mem_stats.live_* == 0` at exit.
- Borrow checker rejects region-escape and cross-region borrow patterns.

### M7 — Explicit ARC types (optional, stdlib + kernel support)

Goal: enable shared ownership where necessary, while keeping ownership the default.

Design constraints:
- Deterministic inc/dec; cycles are either forbidden by construction or require explicit `weak` patterns.
- ARC is opt-in (no implicit sharing); performance costs are explicit in APIs.

Repo changes:
- Add `rc_bytes` / `arc_bytes` (or more general `rc<T>` later) as new types, with:
  - `clone`, `drop`, `as_view` (borrowed view into the shared buffer).
- Extend compiler drop glue to handle ARC types.

Exit criteria:
- ARC objects are leak-free in the absence of cycles; suites can gate peak bytes/allocs.
