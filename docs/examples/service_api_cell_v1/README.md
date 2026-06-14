# service_api_cell_v1

Starter scaffold for the `svc_api_cell_v1` service archetype.

## Purpose

Minimal API service archetype scaffold with Postgres-backed state and an optional S3-compatible object store binding.

See [Service archetype scaffolds](../services/index.md) for the included surfaces, validation steps, and related guides shared by every archetype.

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
