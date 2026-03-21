# service_scheduled_job_v1

Starter scaffold for the `svc_scheduled_job_v1` service archetype.

## Purpose

Minimal scheduled-job service archetype scaffold with a single burst-batch worker cell.

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

- keep the schedule intent in `runtime.schedule`
- keep external dependencies logical with bindings such as `db.primary`, `msg.orders`, or `obj.documents`
- let the runtime materialize cron, timezone, retry, and suspend behavior per target

Migration note:

- if an older service depends on platform cron objects or vendor scheduler ids directly, move that wiring into the deployment layer and keep only the logical scheduled-job description in the service manifest

Related guides:

- [Guide: Provider-agnostic service runtimes and bindings](../../guides/provider-agnostic-services.md)
- [Guide: Migrating services to logical bindings](../../guides/service-binding-migration.md)
