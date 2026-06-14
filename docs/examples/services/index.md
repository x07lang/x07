# Service archetype scaffolds

Shared notes for the `svc_*` service archetype starter scaffolds. Each archetype README links here for the common surfaces and validation steps, and keeps only what is unique to that archetype.

Archetypes:

- [service_api_cell_v1](../service_api_cell_v1/README.md)
- [service_event_consumer_v1](../service_event_consumer_v1/README.md)
- [service_policy_service_v1](../service_policy_service_v1/README.md)
- [service_scheduled_job_v1](../service_scheduled_job_v1/README.md)
- [service_workflow_service_v1](../service_workflow_service_v1/README.md)

## Included surfaces

Every scaffold ships the same surfaces:

- `arch/service/index.x07service.json`
- `arch/manifest.x07arch.json`
- `src/main.x07.json`
- `src/example.x07.json`
- `tests/tests.json`

These examples are intentionally minimal.
They exist to seed `x07 init` and later service tooling.

## Validate the scaffold

Validate the copied scaffold with:

```sh
x07 service validate --manifest arch/service/index.x07service.json
```

## Related guides

- [Guide: Provider-agnostic service runtimes and bindings](../../guides/provider-agnostic-services.md)
- [Guide: Migrating services to logical bindings](../../guides/service-binding-migration.md)
