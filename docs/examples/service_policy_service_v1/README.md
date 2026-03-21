# service_policy_service_v1

Starter scaffold for the `svc_policy_service_v1` service archetype.

## Purpose

Minimal policy-service scaffold for decision/rules APIs with Postgres-backed policy data.

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

- expose decision logic through the policy-service cell kind
- keep policy data and review state on logical bindings such as `db.primary`, `msg.decisions`, or `obj.policies`
- treat rollout, probes, and autoscaling as runtime hints instead of provider-specific service objects

Migration note:

- if a service currently mixes provider IAM policy identifiers or direct storage endpoints into the decision path, move them behind logical bindings and keep the service interface stable

Related guides:

- [Guide: Provider-agnostic service runtimes and bindings](../../guides/provider-agnostic-services.md)
- [Guide: Migrating services to logical bindings](../../guides/service-binding-migration.md)
