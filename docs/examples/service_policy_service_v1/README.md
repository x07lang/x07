# service_policy_service_v1

Starter scaffold for the `svc_policy_service_v1` service archetype.

## Purpose

Minimal policy-service scaffold for decision/rules APIs with Postgres-backed policy data.

See [Service archetype scaffolds](../services/index.md) for the included surfaces, validation steps, and related guides shared by every archetype.

Reference pattern:

- expose decision logic through the policy-service cell kind
- keep policy data and review state on logical bindings such as `db.primary`, `msg.decisions`, or `obj.policies`
- treat rollout, probes, and autoscaling as runtime hints instead of provider-specific service objects

Migration note:

- if a service currently mixes provider IAM policy identifiers or direct storage endpoints into the decision path, move them behind logical bindings and keep the service interface stable
