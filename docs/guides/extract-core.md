# Guide: Extract core migration

“Extract core” means moving the deterministic domain kernel of an existing service into X07 while keeping the effectful shell (HTTP, DB, queues) thin and replaceable.

## Canonical steps

### 1) Identify the core boundary

Pick a slice that is:

- high-value
- stable enough to specify
- small enough to test thoroughly

Write down:

- inputs
- outputs
- invariants
- error classes

### 2) Implement the kernel first

Rules for the kernel:

- deterministic
- no I/O
- no retries
- explicit error values

Add unit tests that cover:

- happy paths
- invalid inputs
- conflict cases
- idempotency key derivation (when applicable)

### 3) Wrap it with a minimal shell

Shell responsibilities:

- parsing and validation (transport-specific)
- correlation (`request_id`, optional `trace_id`)
- effect execution + explicit retry boundaries
- persistence + idempotency

Keep the shell dumb: the kernel decides; the shell executes.

### 4) Partial migration stop points

Safe stop points (you can ship each):

- kernel-only: used for offline validation
- shadow mode: kernel computes decision, legacy shell executes
- dual write/read: kernel backed by new storage but legacy remains source of truth
- full cutover: shell calls kernel and executes effects via new path

### 5) Prove replay safety

Before increasing concurrency or enabling automated rollbacks:

- simulate duplicates (requests and messages)
- prove the idempotency boundary
- ensure correlation fields are present on failures

## Example

See: `docs/examples/extract-core/`.

## Related docs

- `docs/guides/kernel-shell-production.md`
- `docs/guides/scaling-retry-idempotency.md`
- `docs/x07import/porting-by-example.md`

## Expert notes

For strict boundaries, add an architecture manifest and enforce it with `x07 arch check`.

If you need to carry a legacy protocol across the boundary, isolate it in the shell and translate into a kernel-native data model early.
