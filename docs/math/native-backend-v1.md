# Native math backend (libx07_math)

This document describes the **native** implementation strategy for the external
math package (`x07-ext-math`).

## What is pinned

The C ABI between `x07c`-emitted code and the math backend is pinned by:

- `crates/x07c/include/x07_math_abi_v1.h`

Backends must export the symbols declared in that header.

## Required runtime hooks

The math backend does not own memory. It allocates result `bytes` using the
X07 runtime allocator:

- `ev_bytes ev_bytes_alloc(uint32_t len)`

The backend may trap (abort) on invariant violations:

- `void ev_trap(int32_t code)`

## Building

Use:

- `scripts/build_ext_math.sh`

This builds `crates/x07-math-native` and stages the outputs into:

- `deps/x07/libx07_math.a`
- `deps/x07/include/x07_math_abi_v1.h`

## CI

- `scripts/ci/check_math_smoke.sh`

Runs the smoke suites:

- `ci/suites/smoke/math-f64-bits-smoke.json` (pure)
- `ci/suites/smoke/math-f64-api-smoke.json` (pure, broader API coverage)
- `ci/suites/smoke/math-f64-libm-smoke.json` (run-os)
