# Architecture check

`x07 arch check` verifies a repo’s X07 module graph against a pinned architecture manifest.

It is deterministic and emits machine-readable diagnostics (`x07diag`) plus optional JSON Patch suggestions for repair loops.

## Files

Supported schema versions:

- Manifest: `arch/manifest.x07arch.json` (`schema_version: "x07.arch.manifest@0.1.0"`)
- Lock: `arch/manifest.lock.json` (`schema_version: "x07.arch.manifest.lock@0.1.0"`)

## Command

- `x07 arch check`
- `x07 arch check --manifest arch/manifest.x07arch.json`
- `x07 arch check --lock arch/manifest.lock.json`
- `x07 arch check --format json|text`
- `x07 arch check --out <path>` (writes report; stdout when omitted)
- `x07 arch check --emit-patch <path>` (writes a multi-file JSON Patch set)
- `x07 arch check --write` (applies suggested patches deterministically)
- `x07 arch check --write-lock` (writes/updates `arch/manifest.lock.json`)

Budgets:

- `--max-modules N`
- `--max-edges N`
- `--max-diags N`
- `--max-bytes-in N`

## Module scan

The checker scans for module files (`*.x07.json`) under `--repo-root` and extracts:

- `module_id`
- `imports[]`
- `decls[]` (used for some contract checks, like `contracts.smoke_entry`)

Scan configuration comes from:

- `arch/manifest.lock.json` `module_scan.include_globs` / `exclude_globs` when a lock is used, otherwise
- defaults:
  - include: `**/*.x07.json`
  - exclude: `.git/`, `dist/`, `gen/`, `node_modules/`, `target/`, `tmp/` (and a few common OS-specific junk files)

## Nodes and assignment

`manifest.nodes[]` defines named architecture groups (“nodes”) over modules.

A module belongs to a node if it matches **any** of:

- `match.module_prefixes[]` (prefix match against `module_id`)
- `match.path_globs[]` (glob match against the module’s repo-relative path)

With `checks.deny_orphans=true`:

- modules matching 0 nodes → `E_ARCH_NODE_ORPHAN_MODULE`
- modules matching >1 nodes → `E_ARCH_NODE_OVERLAP_MODULE`

## Checks

### External import allowlist

If an import is not an internal module, it must match:

- one of `externals.allowed_import_prefixes`, or
- one of `externals.allowed_exact`

Violations: `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED`.

### Node import policy (prefix deny/allow)

Each node has an import policy:

- `imports.deny_prefixes`: any import starting with one of these prefixes is an error.
  - Violations: `E_ARCH_IMPORT_PREFIX_DENIED`.
- `imports.allow_prefixes`: every import must start with one of these prefixes.
  - Violations: `E_ARCH_IMPORT_PREFIX_NOT_ALLOWED`.

These checks apply to both internal and external imports.

### Visibility (Bazel-style)

If `checks.enforce_visibility=true` and a node has `visibility.mode="restricted"`, then any node depending on it must be listed in `visibility.visible_to[]`.

Violations: `E_ARCH_VISIBILITY`.

### Rules: deps, layers, cycles

Rules are declared in `manifest.rules[]`:

- `deps_v1` rules
  - `mode="deny"`: forbidden node edge → `E_ARCH_DEPS_DENY`
  - `mode="allow"`: explicitly allowed node edges (used by allowlist mode)
- `layers_v1` rules (`direction="down"`)
  - violations → `E_ARCH_LAYERS_VIOLATION`
- `deny_cycles_v1` rules (`scope="nodes"`) when `checks.deny_cycles=true`
  - violations → `E_ARCH_CYCLE`

### Allowlist mode (v1.1)

If `checks.allowlist_mode.enabled=true` and `default_allow_internal=false`, then every internal node edge must be allowed by:

- a `deps_v1` allow rule, or
- a `layers_v1` rule

Violations: `E_ARCH_EDGE_NOT_ALLOWED`.

### Worlds (v1 + v1.1)

Each node declares a `world` (one of X07’s worlds, for example `solve-pure` or `run-os-sandboxed`).

If `checks.world_of_imported_v1.enabled=true`, `solve-*` nodes must not depend on `run-os*` nodes.

Violations: `E_ARCH_WORLD_EDGE_FORBIDDEN`.

### Smoke entry contract (v1.1)

If a node sets:

```json
{ "contracts": { "smoke_entry": "app.core.smoke_v1" } }
```

…the checker verifies that `smoke_entry` is exported by at least one module assigned to that node.

Violations: `E_ARCH_SMOKE_MISSING`.

### Brand boundary checks (v1.1)

If enabled, “public” nodes must not export functions that accept/return raw `bytes` without a brand (for boundary encodings).

Violations: `E_ARCH_PUBLIC_BYTES_UNBRANDED`.

### Contracts: rr / sm / budgets (`contracts_v1`)

If `manifest.contracts_v1` is present, `x07 arch check` can also validate repo-local “contracts as data” under `arch/`:

- `contracts_v1.rr` (record/replay contracts)
  - validates the rr index and referenced policies/sanitizers
  - can require rr-aware OS calls to be inside a `std.rr.with_policy_v1` scope (`require_policy_for_os_calls=true`)
  - see: [Record/replay](../worlds/record-replay.md)
- `contracts_v1.sm` (state machine contracts)
  - validates the SM index and referenced SM spec files
  - can require generated machine modules under `gen_dir` to be present and up to date (`require_gen_uptodate=true`)
  - see: [State machines](state-machines.md)
- `contracts_v1.budgets` (budget contracts)
  - validates the budgets index and referenced profile files
  - can require selected boundaries to be wrapped in `budget.scope_from_arch_v1` (`require_scopes=true`)
  - see: [Budget scopes](../language/budget-scopes.md)
- `contracts_v1.stream_plugins` (stream plugin contracts)
  - validates the stream plugins index and referenced plugin spec files
  - locks the registry into `arch/contracts.lock.json`
  - verifies `std.stream.xf.plugin_v1` usage references declared plugins (and is allowed in the module’s world)
- `contracts_v1.archive` (archive policy contracts)
  - validates `arch/archive/index.x07archive.json` and referenced profile files under `arch/archive/profiles/`
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.db` (database contracts)
  - validates `arch/db/index.x07db.json`, referenced migration plans, and referenced SQL file hashes
  - validates referenced DB query catalogs under `arch/db/queries/`
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.obs` (observability contracts)
  - validates `arch/obs/index.x07obs.json`, referenced metrics registry, and exporter profiles
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.net` (network protocol contracts)
  - validates `arch/net/index.x07net.json` and referenced gRPC services catalog
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.web` (web API + OpenAPI contracts)
  - validates `arch/web/index.x07web.json`, referenced API catalogs, and OpenAPI profiles
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.crawl` (crawl policies contracts)
  - validates `arch/crawl/index.x07crawl.json` and referenced crawl policy files
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.msg` (messaging core contracts)
  - validates `arch/msg/index.x07msg.json` and referenced messaging driver catalogs
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.msg_kafka` (Kafka driver contracts)
  - validates `arch/msg/kafka/index.x07kafka.json` and referenced profile files
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.msg_amqp` (AMQP 0-9-1 driver contracts)
  - validates `arch/msg/amqp/index.x07amqp.json` and referenced profile/topology files
  - verifies referenced budget profiles exist under `arch/budgets/`
- `contracts_v1.cli` (CLI UX contracts)
  - validates `arch/cli/index.x07cli.json` and referenced profile files
- `contracts_v1.crypto` (crypto/auth contracts)
  - validates `arch/crypto/index.x07crypto.json` and referenced key material files
  - validates referenced JWT profiles (when declared under the crypto contract group)
- `contracts_v1.canonical_json` (canonical JSON enforcement)
  - enforces canonical JSON at contract file boundaries (for example, JCS / RFC 8785)

## Output

By default, `x07 arch check` prints a report JSON object to stdout:

- `schema_version: "x07.arch.report@0.1.0"`
- `diags[]`: `x07diag` diagnostics (`spec/x07diag.schema.json`)
- `suggested_patches[]`: multi-file JSON Patch suggestions (optional)

Exit codes:

- `0`: pass (no errors)
- `2`: errors found
- `3`: manifest/lock/schema invalid (could not check)
- `4`: tool budget exceeded

## Fixes and patchsets

- The report includes `suggested_patches[]` (multi-file patches).
- `--emit-patch <path>` writes a patchset file:
  - `schema_version: "x07.arch.patchset@0.1.0"`
- `--write` applies suggested patches deterministically and re-runs the check.

## Lock file

The lock file records:

- manifest path (repo-relative)
- `sha256(jcs(manifest))`
- module scan globs (include/exclude)

Use it in CI to detect drift (`E_ARCH_LOCK_MISMATCH`), and update it deterministically with `--write-lock`.

## Schemas

- `spec/x07-arch.manifest.schema.json`
- `spec/x07-arch.manifest.lock.schema.json`
- `spec/x07-arch.report.schema.json`
- `spec/x07-arch.patchset.schema.json`
