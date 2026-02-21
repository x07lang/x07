# Optimizer

X07’s C backend (`x07c`) includes a deterministic, semantics-preserving optimizer that rewrites a subset of common IR patterns to reduce emitted C size and improve downstream compilation.

The optimizer is deliberately conservative: it only performs rewrites with clear, local proofs and stable thresholds. It does not use heuristics or fixed-point iteration.

## Pass order

Passes run in a fixed order:

1. Constant folding
2. Strength reduction (`*`, `/`, `%`)
3. Dead code elimination (unused `let` in `begin`, i32-only, effect-free)
4. Bounded loop unrolling (small `for` ranges)
5. CSE for pure i32 subexpressions
6. LICM for `bytes.len` inside `for`
7. Dead code elimination again (single cleanup pass)

## Determinism guarantees

- Pass order is fixed and does not depend on hash iteration or runtime measurements.
- Thresholds are constants:
  - `INLINE_MAX_NODES = 12`
  - `UNROLL_MAX_ITERS = 8`
- No fixed-point loops: at most one extra DCE cleanup pass.

## Constant folding semantics (i32)

When both operands are integer literals, the optimizer folds:

- Arithmetic: `+`, `-`, `*` (wrapping i32)
- Unsigned division/modulo:
  - `/` by `0` ⇒ `0`
  - `%` by `0` ⇒ numerator
- Bitwise: `&`, `|`, `^` (as u32 bits, cast back to i32)
- Shifts: `<<u`, `>>u` with shift amount masked by `31`
- Comparisons returning i32 `0`/`1`:
  - Signed: `<`, `<=`, `>`, `>=`
  - Unsigned: `<u`, `<=u`, `>u`, `>=u`
  - Equality: `=`, `!=`

## Strength reduction (exact i32 rewrites)

Strength reduction rewrites only when semantics are exact and the rewrite does not drop evaluation of a potentially effectful operand.

- `x * (2^k)` ⇒ `x <<u k`
- `x / (2^k)` ⇒ `x >>u k` (unsigned semantics)
- `x % (2^k)` ⇒ `x & (2^k - 1)` (unsigned semantics)

## DCE: unused `let` removal

Inside a `begin` block, an unused `["let", name, rhs]` is removed only when `rhs` is proven to be an effect-free i32 expression under a conservative local environment:

- literals (`Int`), or
- i32 operator trees over i32 locals and literals.

This intentionally avoids removing allocations, borrows, or other operations that could change resource behavior.

## Conservative called-once inlining

`x07c` performs a small, program-level inlining pass before expression optimization.

A function is eligible only if:

- it is a non-async, non-extern `defn`,
- it has no `requires`/`ensures`/`invariant` clauses,
- params are all `i32` and the result is `i32`,
- the body is a small pure i32 expression tree (`node_count <= 12`),
- it is called exactly once across the whole program,
- caller and callee are in the same module (module id = everything before the last `.`).

Inlining rewrites a call into a `begin` that binds arguments once (via fresh `let` temps) and substitutes those temps into the callee body.

## Bounded `for` unrolling

`["for", var, start, end, body]` is unrolled only when:

- `start` and `end` are integer literals,
- the iteration count under **unsigned** semantics is small (`<= 8`),
- `body` does not assign to `var` and does not contain a nested `for` that reuses `var`.

The rewrite preserves per-iteration scope by wrapping each iteration body in `["begin", body, 0]` and ends with `0` to match the `for` expression’s i32 result.

