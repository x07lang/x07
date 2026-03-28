# Guide: Kernel/shell in production

The kernel/shell split is the simplest way to keep an X07 service reliable:

- **Kernel**: deterministic domain logic (no I/O)
- **Shell**: I/O + retries + idempotency + policy + observability

This guide focuses on the production details: error classification, request correlation, and test boundaries.

## Canonical structure

Use a small, boring module layout:

- `kernel.*` modules
  - no OS access
  - no network/database clients
  - no retries
- `shell.*` modules
  - parse/validate inputs
  - assign/propagate `request_id`
  - perform effects with explicit retry boundaries
  - record outcomes with idempotency keys

Recommended boundaries:

1. **Parse** in shell: decode input, reject malformed early.
2. **Validate + compute intent** in kernel: return a decision object.
3. **Execute effects** in shell: DB writes, calls, publishing.
4. **Commit** in shell before ack: make replay safe.

## Error classification (production rule)

Keep kernel errors structural and stable:

- `invalid_input`
- `conflict`
- `not_found`
- `invariant_violation`

Shell errors should carry transport and dependency details:

- timeouts
- connection failures
- downstream 5xx
- quota/limit enforcement

The shell maps kernel errors to the external contract (HTTP status, message nack, etc).

## Observability contract

Production support depends on consistent correlation fields.

Required fields in the **shell**:

- `request_id` (always present)
- `trace_id` (present when tracing is enabled)

Contract rules:

- propagate `request_id` across async boundaries and retries
- emit `request_id` in responses (headers) and in structured logs
- keep high-cardinality identifiers out of metric labels unless explicitly budgeted

## Boundary tests (what to prove)

Prove the boundary with two test classes:

- **Kernel unit tests** (pure, fast): state transitions, invariants, idempotency key derivation.
- **Shell integration tests** (sandboxed): retry logic, redaction rules, and correlation propagation.

## Anti-patterns

- Kernel performs I/O (“just one DB call”).
- Shell recomputes business rules (“quick validation here”).
- Retries around non-idempotent effects.
- Correlation fields added only on success paths.
- Logging secrets “temporarily” for debugging.

## Related docs

- `docs/guides/x07-service-architecture-v1.md`
- `docs/guides/scaling-retry-idempotency.md`
- `docs/toolchain/testing-by-example.md`

## Expert notes

If you need a strict boundary, enforce it with an architecture manifest:

- allow kernel modules to import only `std.*` (and no OS adapters)
- allow shell modules to import kernel + I/O adapters
- deny cycles

Use `x07 arch check` for the enforcement path. Avoid direct runner invocation unless you are debugging a harness failure.
