# service_event_consumer_v1

Starter scaffold for the `svc_event_consumer_v1` service archetype.

## Purpose

Minimal event-consumer service archetype scaffold using an AMQP-first message lane plus Postgres state.

See [Service archetype scaffolds](../services/index.md) for the included surfaces, validation steps, and related guides shared by every archetype.

Reference pattern:

- keep the bus logical with bindings such as `msg.orders`
- describe the consumer attachment in `runtime.event`
- keep idempotency and retry handling in the service shell, not in provider-specific queue clients
- use `db.primary` or `obj.documents` as additional logical bindings when the consumer needs state or durable payload storage

Migration note:

- if the service still embeds broker queue names or cloud subscription ids, move them into binding materialization and keep only `msg.*` refs in the service manifest
