# Guide: Provider-agnostic service runtimes and bindings

X07 service projects should be authored once and mapped onto different run targets without rewriting application code for each provider.

The canonical split is:

- service code depends on logical bindings and cell kinds
- topology decides placement and target kind later
- adapters map those logical contracts onto hosted, Kubernetes, or wasmCloud runtimes

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

## Canonical authoring rule

Code depends on logical bindings, not providers.

Examples:

- use `db.primary`, not `postgres.production-east`
- use `msg.orders`, not `rabbitmq-cluster-1`
- use `obj.documents`, not `s3://prod-bucket-a`

Provider-specific data belongs in target profiles and binding materialization, not in service code.

## Runtime classes and target kinds

The service manifest expresses the runtime class the cell needs.
The target profile expresses where that class is realized.

Typical mapping:

- `native-http` or `native-worker` cell -> hosted runtime, Kubernetes runtime, or other native target
- `wasm-component` cell -> wasmCloud or another component-capable target
- `embedded-kernel` cell -> colocated helper or in-process policy logic

Typical target kinds:

- `hosted`
- `k8s`
- `wasmcloud`

The same service can keep one manifest and swap target profiles during promotion or conformance work.

## Binding translation

The public shape is:

- service manifest declares `binding_refs`
- topology and target selection resolve those refs into concrete adapters
- runtime-specific adapters publish the final binding view

Examples:

- `db.primary` can resolve to managed SQLite in hosted mode, a Postgres service in Kubernetes, or another compatible database adapter
- `secret.stripe` can resolve to hosted secret storage, Vault, or a cloud secret manager
- `obs.otlp` can resolve to a local collector, a cluster collector, or a hosted OTLP endpoint

This keeps the application boundary stable while the operator changes infrastructure.

## Canonical project flow

1. Start from a service scaffold:

```bash
x07 init --template api-cell
```

2. Define cells and logical bindings in `arch/service/index.x07service.json`.

3. Validate the service manifest:

```bash
x07 service validate --manifest arch/service/index.x07service.json
```

4. Generate constrained authoring inputs when needed:

```bash
x07 service archetypes
x07 service genpack schema --archetype api-cell
x07 service genpack grammar --archetype api-cell
```

5. Package and target the workload with the platform/runtime layer instead of rewriting the service.

## Review checklist

- every external dependency is named by a logical binding
- every cell has one clear ingress kind and runtime class
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
- [Platform for agents](../agent/platform.md)
- [Guide: Messaging and event streaming](messaging.md)
- [Guide: Databases](databases.md)
