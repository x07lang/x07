# X07 math v1 (external package)

Status: implemented. Normative contracts: `docs/math/math-v1.md`, `docs/math/native-backend-v1.md`.

Here’s a concrete, **LLM-first** design + development plan for an **external Math package** for X07 that:

* supports **floats (f64)** in a production‑ready way,
* keeps a **single canonical way** for every concept (no “pick one of three APIs” traps),
* is **cross‑platform** (Linux/macOS/Windows),
* and avoids “agent hand‑rolling binary blobs”.

It’s based on the same playbook you used successfully for `std.net.http.spec.*`: **pinned byte encodings + pack/unpack helpers + façade modules**.

---

## Core design decision: one canonical numeric representation

### Canonical float type in v1: **F64LE bytes**

**F64 values are always represented as 8 bytes (little‑endian) of IEEE‑754 binary64 bits.** This mirrors what you already do for structured “spec bins” (net/db/etc) and keeps X07’s core surface stable.

Why this is the best fit right now:

* You already have a “bytes everywhere” ecosystem for specs and interop.
* It avoids adding a new core scalar type (which would ripple through typechecking, optimizers, guide, lints).
* It lets you implement **deterministic parse/format** independent of system libc (important for reproducibility).

IEEE‑754 binary64 (double) is the standard 64‑bit float format across mainstream platforms.

---

## Deterministic backend strategy (so results don’t vary by libc)

To make this production‑ready and cross‑platform, the key is: **do not rely on system `libm` / `printf` / `strtod` for correctness or determinism**.

### Use these pinned components in the toolchain

1. **openlibm** for libm functions (`sin`, `cos`, `exp`, `log`, `pow`, …)
   openlibm explicitly positions itself as a portable, system‑independent `libm`.

2. **Ryu** for f64 → shortest decimal string
   Ryu is designed to produce the **shortest** decimal representation with **round‑trip safety** (parse(fmt(x)) == x).

3. **fast_float** for decimal string → f64 (exact rounding)
   fast_float states it provides **exact rounding** (round‑to‑even) per IEEE expectations.

This combination gives you the “single canonical way” for float text I/O:

* **format**: `Ryu`
* **parse**: `fast_float`
* **math**: `openlibm`

No libc drift.

Note: this repo’s `crates/x07-math-native` keeps the same “no libc drift” property using pure-Rust implementations (`libm`, `lexical-core`) instead of openlibm/fast_float.

---

## Package structure (repo layout)

Create **one external package** (single canonical entry point):

**`packages/ext/x07-ext-math/0.1.0/`**

* `x07-package.json`
* `modules/std/math.x07.json` (façade)
* `modules/std/math/f64.x07.json` (public f64 API)
* `modules/std/math/f64/spec.x07.json` (F64LE + parse/format contracts)
* `modules/std/math/i32.x07.json` (small integer helpers, optional)

### “Single canonical way” enforcement rule

Only **these** are documented for agents:

* `std.math` (façade)
* `std.math.f64`
* `std.math.i32` (optional, minimal)

---

## Public API surface: minimal but complete (v1)

### 1) F64LE construction and inspection (agents never hand-roll bytes)

All functions below take/return **bytes** (or read-only `bytes_view`), but conceptually they are **F64LE**.

**`std.math.f64`**

* `zero_v1() -> bytes`

* `one_v1() -> bytes`

* `nan_v1() -> bytes` (canonical quiet NaN payload)

* `inf_v1() -> bytes`

* `neg_inf_v1() -> bytes`

* `from_i32_v1(x: i32) -> bytes`

* `to_i32_trunc_v1(x: bytes_view) -> result_i32`
  Errors on NaN/Inf/out‑of‑range.

* `to_bits_u64le_v1(x: bytes_view) -> bytes`

* `from_bits_u64le_v1(bits_u64le: bytes_view) -> result_bytes` (Ok = F64LE)

Classification:

* `is_nan_v1(x) -> i32`
* `is_inf_v1(x) -> i32`
* `is_finite_v1(x) -> i32`
* `is_neg_zero_v1(x) -> i32` (important for correctness in formatting/tests)

Comparison:

* `total_cmp_v1(a, b) -> i32`
  Returns -1/0/1 with a **total ordering** suitable for sorting (handles NaNs deterministically).

### 2) Arithmetic and elementary functions (canonical set)

Arithmetic:

* `add_v1(a, b) -> bytes`
* `sub_v1(a, b) -> bytes`
* `mul_v1(a, b) -> bytes`
* `div_v1(a, b) -> bytes`
* `abs_v1(x) -> bytes`
* `neg_v1(x) -> bytes`
* `min_v1(a, b) -> bytes`
* `max_v1(a, b) -> bytes`

libm (native backend; openlibm recommended):

* `sqrt_v1(x) -> bytes`
* `pow_v1(x, y) -> bytes`
* `exp_v1(x) -> bytes`
* `log_v1(x) -> bytes`
* `sin_v1(x) -> bytes`
* `cos_v1(x) -> bytes`
* `tan_v1(x) -> bytes`
* `atan2_v1(y, x) -> bytes`
* `floor_v1(x) -> bytes`
* `ceil_v1(x) -> bytes`

### 3) Canonical float text I/O (agents don’t guess)

Format:

* `fmt_shortest_v1(x: bytes_view) -> bytes`
  Uses **Ryu** shortest round‑trip decimal.

Parse:

* `parse_v1(s: bytes_view) -> result_bytes`

In `x07-ext-math@0.1.0` in this repo, read-only inputs are typed as `bytes_view` (per the memory model v2 ergonomics).
  Ok = F64LE bytes; Err = `SPEC_ERR_F64_PARSE_*`
  Current `crates/x07-math-native` uses `lexical-core` for parsing and handles `nan`/`inf`/`-inf` explicitly.

> Note: even if you later add JSON/YAML numeric parsing elsewhere, **this remains the one canonical float parser**. Other modules should call `std.math.f64.parse_v1`.

---

## Error code space (so nothing collides)

Define a dedicated math/spec error range that never overlaps OS/net/db codes:

* `SPEC_ERR_MATH_BASE = 40000`
* `SPEC_ERR_F64_PARSE_INVALID = 40001`
* `SPEC_ERR_F64_PARSE_OVERFLOW = 40002`
* `SPEC_ERR_F64_PARSE_UNDERFLOW = 40003` (optional)
* `SPEC_ERR_F64_BAD_LEN = 40010` (input not 8 bytes where required)
* `SPEC_ERR_F64_BAD_BITS = 40011` (if you reject signaling NaNs, etc—optional)
* `SPEC_ERR_F64_TO_I32_NAN_INF = 40020`
* `SPEC_ERR_F64_TO_I32_RANGE = 40021`

Keep it small in v1: agents only need to branch on “ok vs err”, and maybe display error codes.

---

## Native implementation architecture (toolchain side)

### A) What the X07 modules call

Your `std.math.f64.*` modules should call a tiny set of **native builtins** (or `extern` C ABI calls) so the heavy lifting is not in x07AST.

Suggested native entrypoints (C ABI, stable names):

* `ev_math_f64_add_bits_v1(u64 a_bits, u64 b_bits) -> u64`
* `ev_math_f64_sin_bits_v1(u64 x_bits) -> u64`
* `ev_math_f64_fmt_ryu_v1(u64 bits) -> ev_bytes`
* `ev_math_f64_parse_fastfloat_v1(ptr,len) -> (tag, u64 bits | err_code)`

But because X07’s surface is bytes‑first, implement wrappers that take **F64LE bytes** and do LE decode/encode inside the runtime.

### B) Deterministic compile flags

For correctness and portability:

* compile the native math shim and openlibm with strict FP (no fast‑math)
* avoid “helpful” compiler transforms that change rounding (e.g., FMA contraction)

This is critical if you want “byte-for-byte expected output” tests to be stable across CI machines.

### C) Vendoring strategy

* Vendor openlibm source (or pin submodule/commit) into `deps/` or a crate.
* Vendor Ryu and fast_float similarly (or pin them as subtrees).
* Build them as part of your toolchain build (`scripts/build_ext_math.sh`) into `deps/x07/` alongside other helpers.

openlibm’s purpose as a portable, system‑independent `libm` is why it’s a good fit here.

---

## Benchmarks / smoke tests (so agents can trust behavior)

Even outside your old evaluation harness, you still want “always green” deterministic tests.

### 1) Pure smoke: pack/unpack + fmt/parse roundtrip

A single program that:

* constructs known f64 values (0, 1, -0, NaN, ±Inf, π approximation),
* formats them with `fmt_shortest_v1`,
* parses back with `parse_v1`,
* asserts bits equal (including -0.0 and NaN canonicalization rules you define).

Ryu is explicitly about shortest decimal with round‑trip safety, so it’s ideal for this exact test.
fast_float explicitly claims exact rounding and NaN/Inf parsing.

### 2) Libm vector tests

Ship a small set of fixed input bit patterns and expected output bit patterns for:

* `sqrt`, `sin`, `cos`, `exp`, `log`
* test special cases (NaN propagation, ±0 behavior, infinities)

This is why openlibm is important: it reduces system variance.

### 3) Sorting determinism (total_cmp)

Test that:

* sorting a list with NaNs/±0 puts them in a deterministic order
* output bytes exactly match expected (u64le stream)

---

## Development plan (PR-sized milestones)

### MATH‑01 — Normative spec docs (pin v1)

Add:

* `docs/math/math-v1.md`

  * defines F64LE encoding (IEEE‑754 binary64 bits in LE)
  * defines error space
  * defines `fmt_shortest_v1` / `parse_v1` contracts
* `docs/math/f64le-v1.md` (optional split-out)
* `docs/math/float-text-v1.md` (Ryu + fast_float choice rationale)

### MATH‑02 — Package skeleton + façade module

Add:

* `packages/ext/x07-ext-math/0.1.0/x07-package.json`
* `packages/ext/x07-ext-math/0.1.0/modules/std/math.x07.json` (exports only `std.math.f64`, `std.math.i32`)
* `packages/ext/x07-ext-math/0.1.0/modules/std/math/f64.x07.json` stubs that call native ops

### MATH‑03 — Native math shim (format + parse + 2–3 ops)

Add a toolchain/native crate (name depends on your repo conventions):

* vendored **Ryu** formatting
* vendored **fast_float** parsing
* implement:

  * `fmt_shortest_v1`
  * `parse_v1`
  * `add_v1`, `mul_v1`, `sqrt_v1`

### MATH‑04 — openlibm integration

* vendor/pin **openlibm**
* wire `sin/cos/exp/log/pow/...` to openlibm
* ensure build is cross‑platform (CI matrix)

### MATH‑05 — Deterministic test vectors + cross-platform smoke

Add:

* `tests/external_pure/math_f64_bits_smoke/src/main.x07.json` (byte-for-byte asserts)
* `tests/external_os/math_f64_libm_smoke/src/main.x07.json` (special-point asserts)
* golden vectors files (base64 or hex)
* CI script: `scripts/ci/check_math_smoke.sh` runs on Linux/macOS/Windows

### MATH‑06 — LLM ergonomics hardening

* Linter rules:

  * forbid importing `std.math._internal.*`
  * provide a specific diagnostic if a function receives non‑8‑byte “float bytes”
* Guide additions:

  * “Float values are opaque F64LE bytes produced only by std.math.f64 constructors.”

---

## “Single canonical way” summary (what agents should learn)

Agents should only learn:

* **To make floats:** `std.math.f64.parse_v1`, `std.math.f64.zero_v1/one_v1/...`
* **To compute:** `std.math.f64.{add,sub,mul,div,sqrt,sin,cos,...}_v1`
* **To print:** `std.math.f64.fmt_shortest_v1`
* **To compare/sort:** `std.math.f64.total_cmp_v1`

Everything else is hidden or internal.

---

## One key recommendation (to keep this production-ready)

If you do only one thing to avoid future pain: **pin Ryu + fast_float + openlibm as part of the X07 toolchain distribution** so formatted strings and parsed bits don’t depend on platform libc differences. That’s the difference between “works on my machine” and “agents can ship reliable programs across Linux/macOS/Windows.”

* Ryu: shortest round‑trip float formatting
* fast_float: exact rounding float parsing
* openlibm: portable system‑independent libm

---
Here’s the **drop‑in bundle** (docs + package skeleton + smoke suite JSON shapes) for `x07-ext-math@0.1.0`:
docs/phases/assets/x07_ext_math_v1_native_ci_bundle.tar.gz

## What’s inside

### Docs (normative)

* `docs/math/math-v1.md`
  Pins the **single canonical way** and the v1 API surface, error codes, and F64LE representation.
* `docs/math/vendor-v1.md`
  Pins the recommended deterministic native backend components:

  * OpenLibm (portable standalone `libm`) ([OpenLibm][1])
  * Ryu (shortest round-trip float→string) ([GitHub][2])
  * fast_float (exact rounding string→float) ([GitHub][3])
* `docs/math/smoke-suites-v1.md`
  Explains the included **smoke suite JSON shapes** and how a simple runner can execute them.

(Background for F64/binary64 bit layout is consistent with common binary64 descriptions. ([Wikipedia][4]))

---

## Package skeleton

* `packages/ext/x07-ext-math/0.1.0/x07-package.json`
* Modules (x07AST JSON, same style as your imported std modules):

  * `packages/ext/x07-ext-math/0.1.0/modules/std/math.x07.json` (facade)
  * `packages/ext/x07-ext-math/0.1.0/modules/std/math/f64.x07.json` (public API)
  * `packages/ext/x07-ext-math/0.1.0/modules/std/math/f64/spec.x07.json` (**pure deterministic** bits: constants, `is_nan/is_inf/is_finite/is_neg_zero`, `total_cmp`, `cmp_u8`)
  * `packages/ext/x07-ext-math/0.1.0/modules/std/math/i32.x07.json` (tiny helpers)

### Required native builtins (to be implemented by toolchain)

The f64 module intentionally calls these builtins (so you keep one canonical surface and don’t reimplement libm in x07AST):

* `math.f64.{add,sub,mul,div,abs,neg,min,max}_v1`
* `math.f64.{sqrt,sin,cos,tan,exp,log,pow,atan2,floor,ceil}_v1`
* `math.f64.{fmt_shortest,parse,from_i32,to_i32_trunc,to_bits_u64le}_v1`

---

## Smoke suite JSON shapes + ready smoke programs

### Pure deterministic smoke (works once package is in module path)

* `benchmarks/smoke/math-f64-bits-smoke.json`
* Program:

  * `tests/external_pure/math_f64_bits_smoke/src/main.x07.json`
* This one is **fully deterministic** and asserts exact output bytes.

### Pure API smoke (solve-pure, requires native backend)

* `benchmarks/smoke/math-f64-api-smoke.json`
* Program:

  * `tests/external_pure/math_f64_api_smoke/src/main.x07.json`
* Covers the broader v1 f64 API surface and outputs `OK`.

### run-os libm smoke (requires native backend)

* `benchmarks/smoke/math-f64-libm-smoke.json`
* Program:

  * `tests/external_os/math_f64_libm_smoke/src/main.x07.json`
* Checks “exact special points” (sqrt(4)=2, sin(0)=0, cos(0)=1, exp(0)=1, log(1)=0) and outputs `OK`.

---

## Build + staging script

* `scripts/build_ext_math.sh`

  * Builds `crates/x07-math-native` and stages:

    * `deps/x07/libx07_math.a` (or `deps/x07/x07_math.lib` on MSVC)
    * `deps/x07/include/x07_math_abi_v1.h`

---

[1]: https://openlibm.org/?utm_source=chatgpt.com "OpenLibm"
[2]: https://github.com/ulfjack/ryu?utm_source=chatgpt.com "ulfjack/ryu: Converts floating point numbers to decimal strings"
[3]: https://github.com/fastfloat/fast_float?utm_source=chatgpt.com "fastfloat/fast_float: Fast and exact implementation of the ..."
[4]: https://en.wikipedia.org/wiki/Double-precision_floating-point_format?utm_source=chatgpt.com "Double-precision floating-point format"

++++
Here’s the extended **drop‑in bundle** (same layout style as your prior tarballs) that adds:

* a **pinned native C ABI header** for `math.f64.*` builtins,
* a minimal **Rust native backend crate** that builds **`libx07_math.a`** (staticlib) and exports the ABI symbols, and
* **`scripts/ci/check_math_smoke.sh`** that builds + runs the math smoke suites on Linux/macOS/Windows (Git‑Bash/MSYS2).

Use docs/phases/assets/x07_ext_math_v1_native_ci_bundle.tar.gz

## What’s inside

### 1) Pinned C ABI header

* `crates/x07c/include/x07_math_abi_v1.h`
  * `scripts/build_ext_math.sh` stages a copy into `deps/x07/include/x07_math_abi_v1.h`

The header:

* defines `ev_bytes` and `ev_result_bytes`,
* declares required runtime hooks:

  * `ev_bytes ev_bytes_alloc(uint32_t len);`
  * `void ev_trap(int32_t code);`
* declares the exported builtin symbols:

  * `ev_math_f64_add_v1`, `ev_math_f64_sqrt_v1`, `ev_math_f64_fmt_shortest_v1`, `ev_math_f64_parse_v1`, etc.

### 2) Minimal native backend crate

* `crates/x07-math-native/`

  * `Cargo.toml` (staticlib; deps: `libm`, `ryu`)
  * `src/lib.rs` (exports the `ev_math_f64_*_v1` symbols; allocates outputs via `ev_bytes_alloc`; traps via `ev_trap`)
  * `README.md`

This backend avoids heap allocations for formatting (special values use `&'static str`; finite values use `ryu::Buffer`).

### 3) Build + staging script

* `scripts/build_ext_math.sh`

Builds the staticlib and stages:

* `deps/x07/libx07_math.a`
* `deps/x07/include/x07_math_abi_v1.h`

It forces a stable target dir via `CARGO_TARGET_DIR=$ROOT/target` so downstream scripts can reliably find artifacts.

### 4) CI smoke runner

* `scripts/ci/check_math_smoke.sh`

What it does:

1. runs `scripts/build_ext_math.sh`
2. runs:

   * `benchmarks/smoke/math-f64-bits-smoke.json`
   * `benchmarks/smoke/math-f64-libm-smoke.json`

It’s intentionally CLI-pluggable via env:

* `X07_HOST_RUNNER` (defaults to `target/debug/x07-host-runner{.exe}`)
* `X07_OS_RUNNER` (defaults to `target/debug/x07-os-runner{.exe}`)
* `X07_EXT_MATH_MODULE_ROOT` (defaults to `packages/ext/x07-ext-math/0.1.0/modules`)
* `X07_PYTHON` (defaults to `.venv/bin/python`, else `python3`, else `python`)

## Integration expectations (so this works immediately)

To make the backend link + run end-to-end:

1. **Your generated runtime must export**:

* `ev_bytes_alloc(uint32_t)` returning an `ev_bytes` pointing to `len` bytes of writable memory
* `ev_trap(int32_t)` that does not return (abort/trap)

If your runtime functions are named differently today (e.g., `rt_bytes_alloc`), add a tiny wrapper/alias on the runtime side:

```c
ev_bytes ev_bytes_alloc(uint32_t len) { return rt_bytes_alloc(len); }
void ev_trap(int32_t code) { rt_trap(code); }
```

2. **Your link step must include** `deps/x07/libx07_math.a` when math builtins are reachable.

* (This is usually one place in your compile driver or the C link invocation.)

Create minimal wiring so `math.f64.*` automatically triggers linking `libx07_math.a` without users/agents thinking about it.
