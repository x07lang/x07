# Language overview

X07 is a compiled systems language with an agent-first surface.

## Compilation pipeline (high-level)

1. `*.x07.json` (x07AST JSON) is validated and normalized.
2. The compiler lowers x07AST into a small typed IR.
3. The backend emits C.
4. A host toolchain compiles C into a native executable (or static lib).
5. The runner executes it in the chosen world (fixture or OS).

## Modules

X07 uses explicit modules with deterministic resolution.

- Module IDs are dot-separated: `myapp.http.router`
- Filesystem layout is deterministic: `myapp/http/router.x07.json`
- Imports are explicit (no ambient scanning)

## Types

X07 uses a small core type system optimized for agents:

- integers (`i32`, and via packages: `u64`, etc.)
- bytes (owning)
- views/slices (borrowed)
- vectors (`vec_u8`, etc.)
- options/results with stable encodings
- interface records for dynamic dispatch in streaming I/O

See [Types & memory model](types-memory.md).

## Streaming pipes

For deterministic, budgeted streaming pipelines (source → transforms → sink), X07 provides a compiler special form:

- `std.stream.pipe_v1`

See [Streaming pipes](stream-pipes.md).

## Concurrency

X07 provides deterministic async concurrency for fixture worlds and structured concurrency via `task.scope_v1`.

See [Concurrency & multiprocessing](concurrency-multiprocessing.md).

## Budget scopes

To localize performance/cost contracts, X07 provides budget scopes:

- `budget.scope_v1`
- `budget.scope_from_arch_v1` (pinned budgets from `arch/budgets/`)

See [Budget scopes](budget-scopes.md).

## “Single canonical way”

Where many languages offer multiple competing patterns, X07 standardizes:

- one bytes encoding per data structure,
- one module per domain (e.g., `std.net.http.spec`),
- one “do it right” function for common actions (e.g., `req_get_v1` helpers).

This is about **agent reliability**, not minimalism.
