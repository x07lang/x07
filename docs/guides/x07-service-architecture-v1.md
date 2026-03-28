# X07 service architecture v1

This document freezes the first public vocabulary for X07-native backend services.

## Why this exists

Traditional backend architecture debates usually start with **topology**:
monolith, modular monolith, microservices, serverless, event-driven services, and so on.

For X07, that is the wrong starting point.

X07 is designed for **agentic coding**.  
That means the architecture should optimize for:

- deterministic authoring
- small reviewable units
- explicit capabilities and trust boundaries
- topology flexibility
- operational control after deploy

## The four core concepts

### 1. Domain Pack

The **Domain Pack** is the unit of authoring.

A Domain Pack contains:
- pure domain kernels
- schemas and invariants
- state transition logic
- fixtures and deterministic tests
- service manifests
- one or more Operational Cells

Agents should mostly edit Domain Pack code.

### 2. Operational Cell

The **Operational Cell** is the unit of release, scaling, and trust review.

A cell has:
- one ingress kind
- one runtime class
- one scale class
- one capability / binding surface
- one operational entry
- one trust boundary for release evidence

Examples:
- HTTP/API cell
- event-consumer cell
- scheduled-job cell
- policy-service cell
- workflow-service cell
- MCP/tool cell

### 3. Topology Profile

The **Topology Profile** decides how cells are placed at runtime.

A topology profile may:
- colocate multiple cells into one deployable
- split cells into independent deployables
- choose a target kind such as hosted, Kubernetes, or wasmCloud
- choose runtime class preferences such as Wasm component vs native process

Topology is data, not architecture religion.

### 4. Resource Binding

A **Resource Binding** maps a logical dependency name to real infrastructure.

Examples:
- `db.primary`
- `cache.default`
- `msg.orders`
- `obj.documents`
- `secret.stripe`
- `obs.otlp`

Code should depend on logical bindings, not cloud-vendor SDKs.

## Design principles

### Authoring is not deployment

The unit an agent edits is not necessarily the unit that gets scaled.

- **authoring unit** = Domain Pack
- **assurance unit** = Operational Cell
- **runtime unit** = topology group / deployment

### Pure kernel + effectful shell

Every service should prefer:
- a small pure or mostly-pure kernel
- a thin effectful shell around that kernel

This keeps the certifiable surface small and the operational surface explicit.

### Topology late, boundaries early

X07 should push topology choices later than most human-era service platforms.

Cells and bindings should be explicit immediately.  
Co-location vs split deployment should remain a profile-level decision for as long as possible.

## Canonical service tree

Recommended layout for service-oriented X07 projects:

```text
my-service/
  x07.json
  x07.lock.json
  src/
    main.x07.json
    service/
      api.x07.json
      worker.x07.json
      schedule.x07.json
    domain/
      order_core.x07.json
      billing_rules.x07.json
  arch/
    manifest.x07arch.json
    service/
      index.x07service.json
    trust/
      profiles/
        sandboxed_service_v1.json
  policy/
    run-os.json
  tests/
    tests.json
    domain_core.x07.json
```

## Service manifest v1

The service manifest is stored in:

```text
arch/service/index.x07service.json
```

It describes:
- service identity
- cells
- topology profiles
- binding declarations
- trust defaults
- runtime hints for probes, consumer metadata, schedules, rollout, and autoscaling

This is intentionally separate from `x07.json`:
- `x07.json` stays the project/toolchain manifest
- `index.x07service.json` becomes the service/runtime manifest

## Canonical three-cell example

One Domain Pack can expose multiple Operational Cells without turning topology into a design-time commitment:

```json
{
  "schema_version": "x07.service.manifest@0.1.0",
  "service_id": "orders",
  "display_name": "Orders",
  "domain_pack": {
    "id": "orders",
    "display_name": "Orders"
  },
  "cells": [
    {
      "cell_key": "api",
      "cell_kind": "api-cell",
      "entry_symbol": "orders.api.main",
      "ingress_kind": "http",
      "runtime_class": "native-http",
      "scale_class": "replicated-http",
      "binding_refs": ["db.primary", "obj.documents"],
      "topology_group": "frontdoor",
      "runtime": {
        "probes": {
          "readiness": { "probe_kind": "http", "path": "/readyz", "port": 8080 },
          "liveness": { "probe_kind": "http", "path": "/livez", "port": 8080 }
        },
        "rollout": { "strategy": "rolling", "max_unavailable": "25%", "max_surge": "25%" },
        "autoscaling": { "min_replicas": 2, "max_replicas": 6, "target_cpu_utilization": 70 }
      }
    },
    {
      "cell_key": "events",
      "cell_kind": "event-consumer",
      "entry_symbol": "orders.consumer.main",
      "ingress_kind": "event",
      "runtime_class": "native-worker",
      "scale_class": "partitioned-consumer",
      "binding_refs": ["db.primary", "msg.orders"],
      "topology_group": "async",
      "runtime": {
        "event": {
          "binding_ref": "msg.orders",
          "topic": "orders.created",
          "consumer_group": "orders-workers",
          "ack_mode": "manual"
        },
        "probes": {
          "readiness": { "probe_kind": "exec", "command": ["check-consumer", "--ready"] },
          "liveness": { "probe_kind": "exec", "command": ["check-consumer", "--alive"] }
        },
        "rollout": { "strategy": "rolling", "max_unavailable": "1" },
        "autoscaling": { "min_replicas": 1, "max_replicas": 8, "target_inflight": 64 }
      }
    },
    {
      "cell_key": "settlement",
      "cell_kind": "scheduled-job",
      "entry_symbol": "orders.settlement.main",
      "ingress_kind": "schedule",
      "runtime_class": "native-worker",
      "scale_class": "burst-batch",
      "binding_refs": ["db.primary"],
      "topology_group": "async",
      "runtime": {
        "schedule": {
          "cron": "0 */6 * * *",
          "timezone": "UTC",
          "concurrency_policy": "forbid",
          "retry_limit": 3
        }
      }
    }
  ]
}
```

The example is one service, one Domain Pack, and three cells. A dev topology can still co-locate those cells, while a production topology can split them later.

## Operational Cell shape

Each cell should declare:

- `cell_key`
- `cell_kind`
- `entry_symbol`
- `ingress_kind`
- `runtime_class`
- `scale_class`
- `binding_refs`
- `topology_group`
- `runtime`

`runtime` stays additive and target-agnostic:

- `runtime.probes` describes HTTP or exec probes without baking in a provider object model
- `runtime.event` carries the logical bus binding plus topic and consumer-group data for `event-consumer`
- `runtime.schedule` carries cron, retry, and concurrency hints for `scheduled-job`
- `runtime.rollout` and `runtime.autoscaling` describe first-wedge rollout and scaling hints for runtimes such as Kubernetes

## Scale classes

The initial enum should be:

- `replicated-http`
- `partitioned-consumer`
- `singleton-orchestrator`
- `leased-worker`
- `burst-batch`
- `embedded-kernel`

## Runtime classes

The initial enum should be:

- `wasm-component`
- `native-http`
- `native-worker`
- `embedded-kernel`

## Binding kinds

The initial binding kinds should be:

- `postgres`
- `mysql`
- `sqlite`
- `redis`
- `kafka`
- `amqp` (first-wave messaging lane for the built-in consumer and workflow archetypes)
- `s3`
- `secret`
- `otlp`

## First-wave archetypes

The first scaffolded archetypes should be:

- `api-cell`
- `event-consumer`
- `scheduled-job`
- `policy-service`
- `workflow-service`

## What is out of scope for v1

- service mesh design
- multi-region placement
- vendor-specific cloud primitives in code
- polyglot workload ingestion
- generic monolith vs microservice guidance

## Stability promise

This document freezes the vocabulary and the public intent.  
It does **not** claim that every downstream CLI, runtime, and platform surface is already stable.

## Expert notes

Use the architecture vocabulary to separate authoring from deployment pressure.

- keep Domain Packs small enough that agents can review them as one unit
- split Operational Cells when ingress, trust, or scaling policies diverge
- postpone topology choices until operator evidence forces a more concrete deployment shape
