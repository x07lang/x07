# service_event_consumer_v1

Starter scaffold for the `svc_event_consumer_v1` service archetype.

## Purpose

Minimal event-consumer service archetype scaffold using an AMQP-first message lane plus Postgres state.

## Included surfaces

- `arch/service/index.x07service.json`
- `arch/manifest.x07arch.json`
- `src/main.x07.json`
- `src/example.x07.json`
- `tests/tests.json`

This example is intentionally minimal.
It exists to seed `x07 init` and later service tooling.

Validate the copied scaffold with:

```sh
x07 service validate --manifest arch/service/index.x07service.json
```

Reference pattern:

- keep the bus logical with bindings such as `msg.orders`
- describe the consumer attachment in `runtime.event`
- keep idempotency and retry handling in the service shell, not in provider-specific queue clients
- use `db.primary` or `obj.documents` as additional logical bindings when the consumer needs state or durable payload storage

Migration note:

- if the service still embeds broker queue names or cloud subscription ids, move them into binding materialization and keep only `msg.*` refs in the service manifest

Related guides:

- [Guide: Provider-agnostic service runtimes and bindings](../../guides/provider-agnostic-services.md)
- [Guide: Migrating services to logical bindings](../../guides/service-binding-migration.md)
