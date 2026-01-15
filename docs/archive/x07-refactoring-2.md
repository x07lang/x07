# X07 refactoring 2: stdlib v0.1.1 + range builtins

This refactor focuses on:

1. Fast, explicit “append a byte range” operations (important for Phase‑F mem scoring and for LLM ergonomics).
2. Fast, deterministic byte-range comparison (needed by sorted deterministic data structures and JSON key sorting).
3. Removing redundant aliases that increase LLM confusion.

## Implemented changes

### Builtins

- `(vec_u8.extend_bytes_range h b start len)` -> i32
  - Appends `b[start..start+len]` into vec `h` (bounds-checked; traps on OOB).
  - Uses a single memcpy path (counted in `mem_stats.memcpy_bytes`).

- `(bytes.cmp_range a a_off a_len b b_off b_len)` -> i32
  - Lexicographic comparison of two byte ranges, returning `-1`, `0`, or `1` (memcmp + length tiebreak).
  - Bounds-checked; traps on OOB.

Removed aliases (canonical forms only):

- `vec_u8.new` → use `vec_u8.with_capacity`
- `vec_u8.into_bytes` → use `vec_u8.as_bytes`
- `str.len`, `str.slice`, `str.eq` → strings are just `bytes` (use `bytes.*` + `std.text.*`)

### Stdlib (`stdlib/std/0.1.1/`)

- Compiler-embedded stdlib bumped to `stdlib/std/0.1.1/`.
- `std.text.ascii` and `std.json` use `vec_u8.extend_bytes_range` instead of nested `vec_u8.extend_bytes(bytes.slice ...)`.
- `std.slice.cmp_bytes` delegates to `bytes.cmp_range` (removes per-byte loop overhead).
- JSON extraction no longer allocates when skipping non-matching values:
  - added scan-only `std.json._skip_value`
  - `std.json.extract_path_canon_or_err` uses it for skipping and for root validation
- `std.csv` module removed (it was only an alias); use `std.result.chain_sum_csv_i32`.

## Migration notes

- Replace `vec_u8.new` with `vec_u8.with_capacity`.
- Replace `vec_u8.into_bytes` with `vec_u8.as_bytes`.
- Replace `std.csv.sum_i32_status_le` with `std.result.chain_sum_csv_i32`.
- Prefer `vec_u8.extend_bytes_range` for copying subranges into an output vec.
- Prefer `std.slice.cmp_bytes` for lexicographic compare (it uses `bytes.cmp_range`).

## Rationale

`bytes.slice` is already a zero-copy view in X07; the remaining footgun in stdlib code was the nested “slice then extend” pattern (extra AST/fuel and error-prone for LLM-generated code). Introducing explicit range builtins provides a single, fast, deterministic path while keeping mem-copy accounting consistent with Phase‑F scoring.

## Verification

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `python3 benchmarks/solve-pure/generate_phaseE_suite.py --check`
- `python3 benchmarks/solve-pure/generate_phaseE_stdlib_suite.py --check`
- `./scripts/ci/check_phases_ad.sh`
