# Task policy graph (`x07.arch.tasks.index@0.1.0`)

X07 can track a project’s runtime task policy as a reviewable `arch/**` artifact.

The canonical file is:

- `arch/tasks/index.x07tasks.json`

This task graph is intended to support:

- structured recovery behavior (skip/retry/fail-fast) for non-critical tasks, and
- consistent recovery event logging.

## File shape

```json
{
  "schema_version": "x07.arch.tasks.index@0.1.0",
  "tasks": [
    {
      "id": "warm_cache",
      "fn": "app.tasks.warm_cache",
      "deps": ["fetch_user_profile"],
      "policy": { "criticality": "optional_v1", "on_failure": "skip_v1" }
    }
  ]
}
```

Fields:

- `tasks[*].id`: stable task id (unique in the file).
- `tasks[*].fn`: fully-qualified function symbol (best-effort mapping to runtime work).
- `tasks[*].deps`: task ids that must run first (DAG edges).
- `tasks[*].policy.criticality`: `critical_v1` or `optional_v1`.
- `tasks[*].policy.on_failure`: `fail_fast_v1`, `skip_v1`, or `retry_bounded_v1`.
- `tasks[*].policy.retry_max`: required for `retry_bounded_v1` (max retry attempts).

## Validation

`x07 arch check` validates the task index when it is referenced by `arch/manifest.x07arch.json`:

- `contracts_v1.tasks.index_path = "arch/tasks/index.x07tasks.json"`

Validation checks include:

- unique task ids,
- `deps` refer to existing tasks,
- no dependency cycles,
- valid `fn` symbol shape, and
- policy consistency (`retry_max` required for `retry_bounded_v1`).

## Execution

`x07 xtal tasks run` executes tasks in dependency order for an incident input:

- `x07 xtal tasks run --input target/xtal/violations/<id>/violation.json`

It generates small wrapper programs under `target/xtal/tasks/<id>/` and runs them via `x07 run` using the incident’s world + input.

If recovery events are enabled (see `docs/toolchain/xtal.md`), it also appends `x07.xtal.recovery_event@0.1.0` entries under `target/xtal/events/<id>/events.jsonl`.
