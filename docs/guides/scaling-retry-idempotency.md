# Guide: Scaling, retry, and idempotency for services

X07 service workloads should scale by adding more cells or more replicas, not by making request handling ambiguous.

The safe default is:

- pure kernel
- thin effectful shell
- explicit retry boundary
- explicit idempotency key when an action can be repeated

## Scale the right unit

Scale the Operational Cell, not the whole Domain Pack.

Use the scale class that matches the work:

- `replicated-http` for stateless request handlers
- `partitioned-consumer` for ordered stream work
- `singleton-orchestrator` for one coordinator at a time
- `leased-worker` for pull-based background work
- `burst-batch` for scheduled or queue-drained jobs
- `embedded-kernel` for colocated helper logic

If two parts of a service need different retry or scaling rules, split them into separate cells.

## Canonical flow

The default retry flow is:

1. compute intent in the pure kernel
2. assign or receive an idempotency key
3. perform the effect
4. record completion before acknowledging success upstream

## Retry rules

Retry only around effectful boundaries.

Good retry targets:

- outbound HTTP calls
- message publish or consume acknowledgements
- database connection acquisition
- object storage writes

Bad retry targets:

- pure validation
- deterministic state transitions
- code paths that already committed an irreversible external effect without an idempotency key

## Idempotency rules

Any mutation that can be replayed by transport, operator action, or rollout should have a stable idempotency key.

Common choices:

- incoming request id from a trusted caller
- message id from the broker envelope
- scheduled execution key derived from the schedule window
- platform release or deployment id

The idempotency key should identify the business action, not the network attempt.

Good:

- `charge:acct_42:invoice_2026_03`
- `settlement:2026-03-19`
- `email:welcome:user_123`

Bad:

- random UUID per retry attempt
- raw timestamp with no business identity

## HTTP/API cells

For `api-cell` workloads:

- keep handlers stateless where possible
- make writes idempotent with a request key or resource version check
- retry outbound calls only when the remote contract is safe for replay
- surface duplicate requests as success if the prior action already completed

## Consumer cells

For `event-consumer` workloads:

- treat message delivery as at-least-once unless the broker contract proves otherwise
- persist dedupe state before acknowledging the message
- partition by the domain key when order matters
- move poison messages to a dead-letter or incident path instead of retrying forever

## Scheduled jobs

For `scheduled-job` workloads:

- derive the idempotency key from the schedule window and job identity
- split long-running fan-out work into smaller child actions when needed
- make reruns safe before increasing concurrency

## Operator checklist

- scale class matches the actual work pattern
- retry loops live only at the effect boundary
- the idempotency key survives process restarts and rollout restarts
- duplicate delivery is a tested case
- success is recorded before the upstream ack when replay is possible

## Expert notes

Treat retries, leases, and partitioning as separate controls.

- increase replicas only after the idempotency boundary is proven safe under duplicate delivery
- use partition keys when order matters more than raw throughput
- move non-idempotent fan-out behind a durable outbox before turning up concurrency

## Related docs

- [X07 service architecture v1](x07-service-architecture-v1.md)
- [Guide: Provider-agnostic service runtimes and bindings](provider-agnostic-services.md)
