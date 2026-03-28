# service_api_cell_v1

Starter scaffold for the `svc_api_cell_v1` service archetype.

## Purpose

Minimal API service archetype scaffold with Postgres-backed state and an optional S3-compatible object store binding.

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

If you wire the optional object-store binding locally, provide the native S3 backend environment:

```sh
export X07_OS_OBJ_S3_ENDPOINT=http://127.0.0.1:9000
export X07_OS_OBJ_S3_BUCKET=svc-api-cell
export X07_OS_OBJ_S3_ACCESS_KEY=minio
export X07_OS_OBJ_S3_SECRET_KEY=minio123
```

Reference pattern:

- keep API state on `db.primary`
- keep optional document or asset storage on `obj.documents`
- decode object-store responses with `std.obj.spec.*`
- keep probe, rollout, and autoscaling intent in the service manifest rather than in provider-specific YAML

Related guides:

- [Guide: Provider-agnostic service runtimes and bindings](../../guides/provider-agnostic-services.md)
- [Guide: Migrating services to logical bindings](../../guides/service-binding-migration.md)
