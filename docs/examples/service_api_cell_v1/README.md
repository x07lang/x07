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
