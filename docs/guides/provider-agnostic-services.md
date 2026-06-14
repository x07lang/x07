# Guide: Provider-agnostic service runtimes and bindings

X07 service projects should be authored once and mapped onto different run targets without rewriting application code for each provider.

This page is the overview. Two companion guides own the detail:

- [X07 service architecture v1](x07-service-architecture-v1.md) is the canonical vocabulary: Domain Packs, Operational Cells, Topology Profiles, Resource Bindings, and the runtime-class / scale-class / target-kind enums.
- [Guide: Migrating services to logical bindings](service-binding-migration.md) is the canonical how-to for moving an existing service off provider-specific wiring.

The canonical split is:

- service code depends on logical bindings and cell kinds
- topology decides placement and target kind later
- adapters map those logical contracts onto hosted, Kubernetes, or wasmCloud runtimes

## Canonical authoring rule

Code depends on logical bindings, not providers.

Examples:

- use `db.primary`, not `postgres.production-east`
- use `msg.orders`, not `rabbitmq-cluster-1`
- use `obj.documents`, not `s3://prod-bucket-a`

Provider-specific data belongs in target profiles and binding materialization, not in service code.

## What stays stable

These inputs should remain stable across targets:

- `arch/service/index.x07service.json`
- logical binding names such as `db.primary`, `msg.orders`, `obj.documents`, `secret.stripe`, `obs.otlp`
- cell intent such as `api-cell`, `event-consumer`, `scheduled-job`, `policy-service`, `workflow-service`
- pure kernels and service entry modules

These inputs should not hard-code a provider:

- Kubernetes object names
- cloud-vendor SDK identifiers
- bucket ARNs
- secret-manager product names
- queue URLs

The service manifest expresses the runtime class a cell needs; the target profile expresses where that class is realized. For the runtime-class and target-kind mapping, see [X07 service architecture v1](x07-service-architecture-v1.md).

## How a logical binding becomes a provider

The public shape is:

- the service manifest declares `binding_refs`
- topology and target selection resolve those refs into concrete adapters
- runtime-specific adapters publish the final binding view

So `db.primary` can resolve to managed SQLite in hosted mode or a Postgres service in Kubernetes, `secret.stripe` can resolve to hosted secret storage, Vault, or a cloud secret manager, and `obj.documents` can stay the logical object-store binding while target materialization picks the concrete S3-compatible endpoint. The application boundary stays stable while the operator changes infrastructure.

When you are converting an existing service from direct provider wiring to logical bindings, follow the step-by-step migration in [Guide: Migrating services to logical bindings](service-binding-migration.md). It covers the object-store wedge, the native S3 environment variables, the messaging and schedule split, and the verification checklist.

## Review checklist

- every external dependency is named by a logical binding
- every cell has one clear ingress kind and runtime class
- every `event-consumer` defines `runtime.event`, and every `scheduled-job` defines `runtime.schedule`
- provider names appear only in target profiles, adapter configuration, or operator docs
- topology decisions are data, not hard-coded branches in application code
- retries and idempotency are handled in the shell around the pure kernel

## Expert notes

Keep provider-specific escape hatches at the topology and adapter layer.

- add a target-specific binding only when the logical binding no longer preserves the contract you need
- keep the service manifest stable even if one target needs more operational metadata than another
- treat hosted, Kubernetes, and wasmCloud mappings as deployment policy, not application identity

## Related docs

- [X07 service architecture v1](x07-service-architecture-v1.md)
- [Guide: Migrating services to logical bindings](service-binding-migration.md)
- [Guide: Messaging and event streaming](messaging.md)
- [Guide: Databases](databases.md)
- [Platform for agents](../agent/platform.md) (archived in the 2026-06 refocus)
