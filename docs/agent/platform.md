# Platform (x07lp)

The x07 Platform (x07-LP) is the public runtime and control-plane surface for X07 workloads and release operations. In the current split, `x07` owns authoring and service scaffolds, `x07-wasm` emits workload, app, and device artifacts, and `x07lp` operates workloads, bindings, releases, incidents, and regressions through the public `lp.*` contracts. The managed control layer built on top of that split is sold as x07 Sentinel; the private repo that implements it is `x07-platform-cloud`.

## Vision

- Turn X07 delivery into a contract-first, machine-readable loop from workload packaging through release, deploy, incident capture, and regression generation.
- Lead with governed backend workloads on Kubernetes while keeping the same public contracts usable for local demos, self-hosted targets, and hosted review flows.
- Keep authoring, packaging, and operations visibly separate: `x07` writes software, `x07-wasm` packages it, and `x07lp` operates it.

## Audience

- service authors who want a default path for API, event-consumer, and scheduled-job cells without hard-coding provider details
- operators and platform teams who need deterministic reports, policy and SLO gates, release evidence, and a clear query and control surface

## Current goals

- maintain a strong public OSS baseline for local, self-hosted, and Kubernetes-backed workload delivery
- keep provider support behind workload, binding, target, and conformance surfaces instead of baking cloud logic into the engine
- preserve the public/private split so hosted-only Sentinel concerns such as account, tenancy, metering, billing, and approvals evolve without forking the public platform

Repos:
- `x07lang/x07-platform-contracts`
- `x07lang/x07-platform`
- `x07lang/x07-platform-cloud` (private repo for the managed Sentinel layer)

CLI: `x07lp`  
MCP namespace: `lp.*`

## Current contract line

- workload, topology, binding, target-health, and release contracts are the current public backend-delivery line
- public platform contracts for device release, incident, and regression run on the `0.2.0` line
- deploy, environment, control, and inventory contracts remain on their current public `0.1.x` lines
- the official MCP lifecycle path is `io.x07/x07lang-mcp` via `lp.query_v1` and `lp.control_v1`
- use the repo release pages for exact current `x07-platform`, `x07-wasm-backend`, `x07-web-ui`, and `x07lang-mcp` bundle tags

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
  - `lp.incident.bundle@0.2.0`
  - `lp.incident.bundle.meta.local@0.2.0`
  - `lp.incident.bundle.meta.remote@0.2.0`
  - `lp.incident.query.result@0.2.0`
  - `lp.regression.request@0.2.0`
  - `lp.regression.run.result@0.2.0`
- device release contracts:
  - `lp.device.release.plan@0.2.0`
  - `lp.device.release.execution@0.2.0`
  - `lp.device.release.query.result@0.2.0`
  - `lp.device.release.run.result@0.2.0`
- remote target and adapter contracts:
  - `lp.target.profile@0.1.0`
  - `lp.target.list.result@0.1.0`
  - `lp.remote.capabilities.response@0.1.0`
  - `lp.adapter.capabilities@0.1.0`
  - `lp.adapter.conformance.report@0.1.0`
- workload delivery contracts:
  - `lp.workload.*`
  - `lp.topology.*`
  - `lp.binding.*`
  - `lp.release.*`
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

The `0.2.0` device-native line adds three normalized surfaces that matter to operators:

- release records carry a frozen `native_summary` plus `release_readiness` derived from the sealed device package and `x07-wasm device package --json`
- incidents carry `native_context` with sanitized release, permission, lifecycle, connectivity, and breadcrumb context, plus the native classification set:
  - `native_runtime_error`
  - `native_policy_violation`
  - `native_bridge_timeout`
  - `native_host_crash`
  - `native_permission_blocked`
- regressions carry `native_replay_hints`, `replay_target_kind`, `replay_mode`, and replay synthesis status so incident bundles can round-trip into deterministic native replay fixtures

The companion `x07-wasm` report line is also `0.2.0`:

- `x07.wasm.device.verify.report@0.2.0`
- `x07.wasm.device.package.report@0.2.0`
- `x07.wasm.device.regress.from_incident.report@0.2.0`

## Control-room client contract map

Forge-style clients should keep candidate/workspace state in the client, then resolve lifecycle truth through the public `lp.*` result contracts:

- control mutation receipt: `lp.control.action.result@0.1.0`
  - stable linkage fields: `action_id`, `kind`, `scope`, `approval_state`, `target`, `affected_executions`, `decision.decision_id`, `decision.record`
- deploy detail and deploy-linked incidents: `lp.deploy.query.result@0.1.0`
  - stable linkage fields: `deployment_id`, `run_id`, `target.app_id`, `target.environment`, `status`, `last_incident_id`, `linked_incidents[].incident_id`, `artifacts[]`, `decisions[]`
- environment overview: `lp.environment.list.result@0.1.0`
  - stable linkage fields: `items[].environment_id`, `items[].display_name`, `items[].project_id`, `items[].updated_unix_ms`
- incident inbox/detail: `lp.incident.query.result@0.2.0`
  - stable linkage fields: `incident_id`, `deployment_id`, `release_exec_id`, `regression_id`, `target`, `captured_unix_ms`, `refs`, `trace`, `bundle`, `items[]`
- regression generation result: `lp.regression.run.result@0.2.0`
  - stable linkage fields: `incident_id`, `regression_id`, `incident_status_after`, `replay_mode`, `replay_synthesis_status`, `generated_trace_artifact_refs`, `generated_report_artifact_refs`

This is the intended M5 boundary in `x07`: the core toolchain and docs point clients at the official machine-readable lifecycle outputs, but they do not invent a second release-candidate or incident domain on top of `x07-platform`.

## Runtime surface

`x07-platform` provides:

- `x07lp` commands for hosted login and scope selection (`login`, `whoami`, `org`, `project`, `env`, `context`), hosted release and binding review (`release-submit`, `release-query`, `release-explain`, `release-rollback`, `binding-status`), and workload runtime control (`workload accept|run|query|reconcile|bindings|stop`)
- deterministic local execution with candidate/stable slots, weighted routing, signed control actions, incident capture, regression generation, deploy query rebuilds, and a Kubernetes workload lane for HTTP, event-consumer, and scheduled-job cells
- self-hosted remote deploy parity through `x07lpd`
- a shared Rust driver (`x07lp-driver`) used by the CLI, MCP layer, and daemon so local and remote flows share one execution path
- secondary app and device-release flows remain available for deploy, incident, regression, and native release supervision when that broader lifecycle loop is needed

The current first backend-delivery lane is the Kubernetes workload path under `x07.workload.pack@0.1.0`. The older self-hosted remote reference target still exists under `x07-platform/examples/targets/wasmcloud/`, and the verified remote path uses:

- HTTPS control-plane ingress on `https://localhost:17443`
- authenticated HTTPS OCI registry publishing on `https://localhost:15443`
- OTLP HTTP telemetry
- encrypted server-side secret storage for `lp.impl.secrets.server_store_v1`

The default server-side secret adapter stays on the same public adapter id, but its stored document is encrypted at rest and requires `X07LP_REMOTE_SECRET_MASTER_KEY_FILE` to resolve secrets.

## Repository split

- `x07-platform-contracts`: authoritative public schemas, examples, compatibility waivers, and registry export helpers
- `x07-platform`: public engine, CLI, daemon, MCP, reference adapters, examples, fixtures, and CI
- `x07-platform-cloud`: private hosted-product layer that powers the managed Sentinel offering, including identity, tenancy, billing, metering, and other multi-tenant service concerns

`x07-platform` consumes `x07-platform-contracts` through the `contracts/` submodule. `x07-platform-cloud` consumes both `contracts/` and `platform/` so the hosted layer builds on the public engine instead of forking it.
