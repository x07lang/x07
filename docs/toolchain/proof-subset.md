# Proof-supported subset (quick guide)

This page is a compact guide to the subset that `x07 verify --prove` can turn into proof evidence.

If you need deeper context on how proof artifacts relate to certification, see `docs/toolchain/formal-verification.md`.

## Inputs and results

Supported direct input/result carriers include:

- `i32`, `u32`
- `bytes`, `bytes_view`, `vec_u8`
- `option_i32`, `result_i32`
- `option_bytes*`, `result_bytes*` (including `*_view` variants)

## Brands on byte carriers

Branded byte carriers are supported when the brand id resolves to a reachable validator:

- Carriers: `bytes@B`, `bytes_view@B`, `option_bytes*@B`, `result_bytes*@B`
- Brand resolution: `meta.brands_v1.validate` on a reachable module (from the entry’s import graph)

For public APIs, `bytes_view@brand` is usually the most proof-friendly shape.

## Loop forms and bounds

- `for` loops must use the strict x07AST form: `["for","i",<start:i32>,<end:i32>,<body:any>]`
- Proof is bounded by `--unwind` and input-size bounds (`--max-bytes-len` / `--input-len-bytes`)
- If a loop can execute more iterations than the configured unwind bound, proof will be inconclusive or fail

## Effects and unsupported operations

`x07 verify --prove` rejects targets that contain unsupported heap/pointer effects (for example raw pointer arithmetic and memory intrinsics) in the certifiable pure subset.

Use `x07 verify --coverage` to see support posture and the exact rejection reason codes.

## Recursion

- Self-recursive `defn` targets require `decreases[]` to be certifiable.
- Strong certification profiles reject bounded-recursion “success” unless explicitly disclosed as an assumption in the trust posture.

