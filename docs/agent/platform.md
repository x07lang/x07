# Platform (x07lp)

The x07 Platform (x07-LP) is a contract-first lifecycle engine for sealed X07 artifacts. It consumes `x07.app.pack@0.1.0` plus the related policy, SLO, provenance, incident, and regression artifacts, then produces deterministic deploy, query, control, incident, and regression outputs.

Repos:
- `x07lang/x07-platform-contracts`
- `x07lang/x07-platform`
- `x07lang/x07-platform-cloud`

CLI: `x07lp`  
MCP namespace: `lp.*`

## Current verified line

- public platform contracts remain on `0.1.0`
- `x07-platform` release line: `0.2.2`
- verified upstream dependency line for the self-hosted remote path:
  - `x07 v0.1.68`
  - `x07-wasm-backend v0.1.8`
  - `std-web-ui 0.1.9`
- the `x07` docs bundle that publishes this page is released on `v0.1.69`

## Public contract surface

`x07-platform-contracts` is the source of truth for public `lp.*` JSON Schemas, including:

- deploy state and query contracts:
  - `lp.deploy.execution@0.1.0`
  - `lp.deploy.execution.meta.local@0.1.0`
  - `lp.deploy.execution.meta.remote@0.1.0`
  - `lp.deploy.query.result@0.1.0`
  - `lp.deploy.push.result@0.1.0`
  - `lp.deploy.remote.result@0.1.0`
- incident and regression contracts:
  - `lp.incident.bundle@0.1.0`
  - `lp.incident.bundle.meta.local@0.1.0`
  - `lp.incident.query.result@0.1.0`
  - `lp.regression.request@0.1.0`
  - `lp.regression.run.result@0.1.0`
- remote target and adapter contracts:
  - `lp.target.profile@0.1.0`
  - `lp.target.list.result@0.1.0`
  - `lp.remote.capabilities.response@0.1.0`
  - `lp.adapter.capabilities@0.1.0`
  - `lp.adapter.conformance.report@0.1.0`
- control and inventory contracts:
  - `lp.control.action.result@0.1.0`
  - `lp.app.list.result@0.1.0`

The hardened self-hosted remote profile lives in `lp.target.profile@0.1.0`:

- `base_url` must be `https://...` unless it is loopback `http://localhost` or `http://127.0.0.1`
- `tls.mode` supports `system`, `ca_bundle`, and `pinned_spki`
- `oci_registry` can be paired with:
  - `oci_auth = { kind: "basic", username_ref, password_ref }`
  - `oci_tls = { mode: "system" | "ca_bundle", ca_bundle_path? }`

`lp.remote.capabilities.response@0.1.0` advertises the remote runtime and registry handshake, including:

- `features.authenticated_oci_push`
- `features.registry_tls`

`lp.deploy.execution.meta.remote@0.1.0` preserves the accepted target profile details needed to understand remote query artifacts later, including TLS and OCI auth/trust references.

## Runtime surface

`x07-platform` provides:

- `x07lp` commands for `change`, `deploy`, `target`, `adapter`, `incident`, `regress`, `app`, `platform`, `ui`, and `schema`
- deterministic local execution with candidate/stable slots, weighted routing, signed control actions, incident capture, regression generation, and query/index rebuilds
- self-hosted remote deploy parity through `x07lpd`
- a shared Rust driver (`x07lp-driver`) used by the CLI, MCP layer, and daemon so local and remote flows share one execution path

The current self-hosted reference target is the wasmCloud stack under `x07-platform/examples/targets/wasmcloud/`. The verified creator-facing path uses:

- HTTPS control-plane ingress on `https://localhost:17443`
- authenticated HTTPS OCI registry publishing on `https://localhost:15443`
- OTLP HTTP telemetry
- encrypted server-side secret storage for `lp.impl.secrets.server_store_v1`

The default server-side secret adapter stays on the same public adapter id, but its stored document is encrypted at rest and requires `X07LP_REMOTE_SECRET_MASTER_KEY_FILE` to resolve secrets.

## Repository split

- `x07-platform-contracts`: authoritative public schemas, examples, compatibility waivers, and registry export helpers
- `x07-platform`: public engine, CLI, daemon, MCP, reference adapters, examples, fixtures, and CI
- `x07-platform-cloud`: private hosted-product layer for identity, tenancy, billing, metering, and other multi-tenant service concerns

`x07-platform` consumes `x07-platform-contracts` through the `contracts/` submodule. `x07-platform-cloud` consumes both `contracts/` and `platform/` so the hosted layer builds on the public engine instead of forking it.
