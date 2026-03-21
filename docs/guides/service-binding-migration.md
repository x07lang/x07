# Guide: Migrating services to logical bindings

Use this guide when a service still depends on provider-specific object-store, queue, or secret wiring and you want one manifest that can move across hosted, Kubernetes, and wasmCloud targets.

## Canonical migration flow

Move provider identity out of service code and into binding materialization.

Before:

- hard-coded bucket names or `s3://` URIs in application logic
- cloud-vendor environment variables read directly by the service
- target-specific queue names or secret-manager identifiers embedded in the service manifest

After:

- logical bindings such as `obj.documents`, `msg.orders`, `db.primary`, `secret.stripe`, `obs.otlp`
- service code that talks to the logical contract only
- target profiles and operator wiring that materialize the concrete provider inputs later

## Object-store migration

The canonical first object-store wedge is `ext-obj-core@VERSION` plus
`ext-obj-s3@VERSION`. Pick the concrete versions from the capability map or the
registry catalog before you add them to a project.

1. Add the packages:

```bash
x07 pkg add ext-obj-core@VERSION
x07 pkg add ext-obj-s3@VERSION
```

2. Replace provider-specific storage calls with `std.obj.*` or `std.obj.s3.*`.

3. Keep the service manifest logical:

- add `obj.documents` to `binding_refs`
- keep runtime hints under `runtime.probes`, `runtime.rollout`, `runtime.autoscaling`, `runtime.event`, or `runtime.schedule`
- do not embed bucket names, MinIO endpoints, or cloud-vendor ids in the manifest

4. Materialize the provider at deploy time:

- native S3-compatible targets export `X07_OS_OBJ_S3_ENDPOINT`
- `X07_OS_OBJ_S3_BUCKET`
- `X07_OS_OBJ_S3_ACCESS_KEY`
- `X07_OS_OBJ_S3_SECRET_KEY`
- optionally `X07_OS_OBJ_S3_PREFIX`

5. Validate the service boundary:

```bash
x07 service validate --manifest arch/service/index.x07service.json
```

## Messaging and schedule migration

- `event-consumer` cells should keep the bus logical with bindings such as `msg.orders`
- the event attachment details live under `runtime.event`
- `scheduled-job` cells keep the job logical and place cron, timezone, retry, and suspend details under `runtime.schedule`

That split keeps the workload portable while still exposing enough runtime intent for the deployment layer.

## Verification checklist

- service code contains logical binding names only
- service manifest contains no provider product names
- target-specific secrets or endpoints live outside the service repo
- examples still validate with `x07 service validate`
- object-store reads and writes decode responses with `std.obj.spec.*`

## Expert notes

- keep provider credentials, bucket names, bus subjects, and secret-manager ids in
  target profiles or operator wiring rather than committing them into the service repo
- when migrating incrementally, move one resource family at a time and keep the
  logical binding name stable so rollout diffs stay reviewable
- if a target needs native S3 variables during migration, inject them from the
  deploy layer and keep the service manifest limited to logical `binding_refs`

## Reference patterns

- [API cell with optional object store](../examples/service_api_cell_v1/README.md)
- [Event consumer with logical message binding](../examples/service_event_consumer_v1/README.md)
- [Scheduled job with runtime schedule hints](../examples/service_scheduled_job_v1/README.md)
- [Policy service with stable decision bindings](../examples/service_policy_service_v1/README.md)
- [Provider-agnostic service runtimes and bindings](provider-agnostic-services.md)
