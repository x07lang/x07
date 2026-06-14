# service_scheduled_job_v1

Starter scaffold for the `svc_scheduled_job_v1` service archetype.

## Purpose

Minimal scheduled-job service archetype scaffold with a single burst-batch worker cell.

See [Service archetype scaffolds](../services/index.md) for the included surfaces, validation steps, and related guides shared by every archetype.

Reference pattern:

- keep the schedule intent in `runtime.schedule`
- keep external dependencies logical with bindings such as `db.primary`, `msg.orders`, or `obj.documents`
- let the runtime materialize cron, timezone, retry, and suspend behavior per target

Migration note:

- if an older service depends on platform cron objects or vendor scheduler ids directly, move that wiring into the deployment layer and keep only the logical scheduled-job description in the service manifest
