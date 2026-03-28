# ADR-0001: service/workload model v1

## Status

Accepted

## Context

X07 needs a backend service model that is:
- agent-friendly
- provider-agnostic
- topology-flexible
- compatible with later release assurance

The existing platform surfaces are app/deploy-centric.
We need a vocabulary that supports later runtime and Sentinel work without forcing immediate destructive rewrites.

## Decision

Use the following terms:

- **service** — authoring-facing term
- **workload** — artifact/runtime-facing term
- **Domain Pack** — authoring unit
- **Operational Cell** — release and scale unit
- **Topology Profile** — placement unit
- **Resource Binding** — logical infra dependency unit

Store service metadata in:

```text
arch/service/index.x07service.json
```

Publish runtime artifacts later as:

```text
x07.workload.pack@0.1.0
```

## Consequences

### Positive
- separates authoring from deployment
- fits the current `x07` + `x07-wasm` + `x07-platform` repo split
- gives Sentinel a clean assurance boundary later

### Negative
- introduces a new manifest family
- adds an extra concept layer above current app/deploy flows
- requires compatibility mapping while the platform is still app-centric

## Alternatives rejected

### Reuse `x07.json` for everything
Rejected because project/toolchain and service/runtime concerns will diverge.

### Adopt “microservice” as the primary unit
Rejected because topology should remain a profile-level decision.

### Call everything an app
Rejected because the runtime artifact line needs to generalize beyond current app UI/device history.
