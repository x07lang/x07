# Review & trust artifacts

`x07 review diff` and `x07 trust report` generate deterministic review artifacts for agent-authored changes.

For the public overview of how proofs, capsules, runtime attestation, and certificates fit together, start with [Formal verification & certification](formal-verification.md).

For the smallest certificate-first example, see:

- `docs/examples/verified_core_pure_v1/`
- `docs/examples/trusted_sandbox_program_v1/`
- `docs/examples/trusted_network_service_v1/`
- `docs/examples/certified_capsule_v1/`
- `docs/examples/certified_network_capsule_v1/`

For package-backed examples, see:

- `x07-mcp/docs/examples/trusted_program_sandboxed_local_stdio_v1/` for the strong-profile sandboxed local line
- `x07-mcp/docs/examples/verified_core_pure_auth_core_v1/` for developer/demo proof-summary workflows that depend on a developer-only imported stub path
- `x07-mcp/docs/examples/trusted_program_sandboxed_net_http_v1/` for developer/demo network packaging and capsule flows

## Semantic diff (`x07 review diff`)

```bash
x07 review diff \
  --from <baseline_path> \
  --to <candidate_path> \
  --html-out target/review/diff.html \
  --json-out target/review/diff.json
```

Notes:

- `--from` and `--to` must both be files or both be directories.
- `--mode project` (default) includes project, `arch/`, and policy surfaces.
- `--mode ast-only` restricts scanning to `*.x07.json`.
- `--fail-on` supports CI gates:
  - `world-capability`
  - `budget-increase`
  - `allow-unsafe`
  - `allow-ffi`
  - `proof-coverage-decrease`
  - `recursion-proof-coverage-decrease`
  - `async-proof-coverage-decrease`
  - `summary-downgrade`
  - `assumption-surface-widen`
  - `dev-only-assumption-introduced`
  - `bounded-proof-introduced`
  - `coverage-summary-imported`
  - `operational-entry-diverges`
  - `boundary-relaxation`
  - `trusted-subset-expansion`
  - `capsule-contract-relaxation`
  - `capsule-set-change`
  - `sandbox-policy-widen`
  - `runtime-attestation-regression`
  - `weaker-isolation-enabled`
  - `network-allowlist-widen`
  - `peer-policy-relaxation`
  - `capsule-network-surface-widen`
  - `package-set-change`
  - `lockfile-hash-change`
  - `advisory-allowance-enabled`

JSON schema:

- `spec/x07-review.diff.schema.json`
- `schema_version: "x07.review.diff@0.5.0"`

## Trust report (`x07 trust report`)

```bash
x07 trust report \
  --project x07.json \
  --profile sandbox \
  --out target/trust/trust.json \
  --html-out target/trust/trust.html
```

The trust report aggregates:

- resolved project world/profile/runner/module roots,
- budget caps + detected `budget.scope_*` usage,
- declared policy capabilities + statically used sensitive namespaces,
- nondeterminism flags,
- deterministic SBOM artifact output + component inventory.

### SBOM artifact output

By default, `x07 trust report` generates a deterministic **CycloneDX 1.5 JSON** SBOM file next to the trust report output.

SBOM file naming is derived from the trust report output filename:

- `--out target/trust/trust.json` → `target/trust/trust.sbom.cdx.json`
- `--sbom-format spdx` uses `target/trust/trust.sbom.spdx.json`
- `--sbom-format none` disables SBOM file generation

### Dependency capability policy (sensitive namespaces)

If the project declares dependencies, `x07 trust report` can enforce a dependency capability policy based on the same
**sensitive namespace** scan used in the trust report capabilities section.

Default policy discovery (project root):

- `x07.deps.capability-policy.json`

Override policy path (safe relative path, resolved against the project root):

- `--deps-cap-policy path/to/policy.json`

Example policy (deny network/process by default, allow one dependency):

```json
{
  "schema_version": "x07.deps.capability_policy@0.1.0",
  "policy_id": "no-net-deps",
  "default": {
    "deny_sensitive_namespaces": ["std.os.net", "std.os.process"]
  },
  "packages": [
    {
      "name": "ext-auth-jwt",
      "allow_sensitive_namespaces": ["std.os.fs"]
    }
  ]
}
```

Notes:

- Namespace strings accept either `std.os.net` or `std.os.net.`; they are normalized to the trailing-dot form (`std.os.net.`).
- Missing policy emits `W_DEPS_CAP_POLICY_MISSING` when dependencies are declared.

Optional observed inputs:

- `--run-report <path>`
- `--bundle-report <path>`
- `--x07test <path>`

Strict/fail gates:

- `--strict`
- `--fail-on allow-unsafe|allow-ffi|net-enabled|process-enabled|nondeterminism|sbom-missing|deps-capability`

JSON schema:

- `spec/x07-trust.report.schema.json`
- `schema_version: "x07.trust.report@0.1.0"`

## Trust profile check (`x07 trust profile check`)

```bash
x07 trust profile check \
  --profile arch/trust/profiles/verified_core_pure_v1.json \
  --project x07.json \
  --entry app.main
```

This validates:

- allowed entrypoints,
- allowed worlds,
- language subset restrictions (`defasync`, `extern`, `allow_unsafe`, `allow_ffi`),
- strong-profile operational-entry wiring (`project.operational_entry_symbol` and optional `project.certification_entry_symbol`),
- arch manifest posture (`allowlist_mode`, cycles/orphans/visibility/world-caps),
- boundary index wiring for the certifiable trust surface,
- per-symbol prove evidence requirements, coverage-import policy, and developer-only assumption posture,
- capsule requirements, runtime-attestation requirements, effect-log requirements, and sandbox backend/network posture when the selected profile asks for them.

Current built-in profiles include:

- `arch/trust/profiles/verified_core_pure_v1.json`
- `arch/trust/profiles/trusted_program_sandboxed_local_v1.json`
- `arch/trust/profiles/trusted_program_sandboxed_net_v1.json`
- `arch/trust/profiles/certified_capsule_v1.json`

## Capsule contracts (`x07 trust capsule`)

```bash
x07 trust capsule check \
  --index arch/capsules/index.x07capsule.json \
  --project x07.json
```

```bash
x07 trust capsule attest \
  --contract arch/capsules/capsule.echo.contract.json \
  --module src/echo.x07.json \
  --lockfile x07.lock.json \
  --conformance-report target/capsules/echo.conformance.json \
  --out target/capsules/capsule.echo.attest.json
```

Use these commands to validate the capsule index plus referenced contracts/attestations, and to emit deterministic `x07.capsule.attest@0.2.0` artifacts that `x07 trust certify` can bind into a certificate bundle. Network capsules can now declare peer-policy files plus request/response contract digests, so the attestation covers the reviewed network surface instead of only the local module set.

For package-backed certification, generate dependency-closure evidence before certification:

```bash
x07 pkg attest-closure \
  --project x07.json \
  --out target/cert/dep-closure.attest.json
```

## Trust certify (`x07 trust certify`)

```bash
x07 trust certify \
  --project x07.json \
  --profile arch/trust/profiles/verified_core_pure_v1.json \
  --entry app.main \
  --out-dir target/cert
```

Gates:

- `--fail-on <COND>` forwards to `x07 trust report --fail-on <COND>` (for example `allow-unsafe`, `net-enabled`).
- `--review-fail-on <CATEGORY>` forwards to `x07 review diff --fail-on <CATEGORY>` when `--baseline <PATH>` is provided.
  - If no `--review-fail-on` is provided, the default diff gates remain in effect.
  - When a baseline is provided, `review.diff.txt` is written next to `review.diff.json`/`review.diff.html` as a stable 1-page summary.

The certificate bundle includes:

- `certificate.json`
- `summary.html`
- `boundaries.report.json`
- `schema-derive/*.json` when boundary schemas are referenced
- `verify.coverage.json`
- per-symbol prove reports under `prove/`
- proof summaries referenced from `proof_inventory`
- proof objects and proof-check reports when the active profile requires them
- `tests.report.json`
- `trust.report.json`
- `compile.attest.json`
- capsule attestation references, runtime-attestation references, and effect-log digests when they are present in the observed evidence
- peer-policy references, network capsule inventory, package-set digest, and dependency-closure evidence when the selected profile requires them

Accepted certificates also expose:

- `operational_entry_symbol`
- `formal_verification_scope`
- `proved_symbol_count`, `proved_defn_count`, `proved_defasync_count`
- `entry_body_formally_proved`
- `operational_entry_proof_inventory_refs`
- `capsule_boundary_only_symbol_count`
- `runtime_evidence_only_symbol_count`
- `proof_inventory`
- `proof_assumptions`
- recursive boundedness honesty (`recursive_proof_summary`)
- explicit booleans for bounded-proof or developer-only-assumption dependence
- per-proof acceptance metadata (`proof_check_result`, `proof_check_checker`, `proof_object_digest`)

`x07 trust certify` rejects if support posture regresses, required prove artifacts are missing, required proof objects or proof-check reports are missing, required boundary metadata is missing, boundary-declared smoke/PBT tests do not resolve and pass, schema-derived outputs drift, trust report cleanliness fails, or compile attestation cannot bind the emitted native artifact.
Strong trust profiles additionally reject:

- operational-entry mismatch
- surrogate certification entries
- coverage-only imports
- developer-only imported stubs
- bounded recursion in accepted proof inventory

Certificate schema:

- `spec/x07-trust.certificate.schema.json`
- `schema_version: "x07.trust.certificate@0.7.0"`

For `verified_core_pure_v1`, boundary-referenced schemas are rechecked with `x07 schema derive --check --out-dir .`, so certified projects should derive those schema outputs into the project root.

When a certified entry depends on reviewed imported helpers, `x07 verify --prove` uses the trusted primitive catalog in `catalog/verify_primitives.json` to model those calls in the proof harness. Developer-only imported stubs are available only behind `--allow-imported-stubs`, and accepted strong certificates reject them fail-closed.

Runtime requirement:

- sandboxed certification flows require a supported `run-os-sandboxed` VM backend so the certificate can bind runtime attestation to the observed execution.

## Certificate-first review flow

For `verified_core_pure_v1`, the reviewer flow is:

1. Run `x07 trust certify ... --out-dir target/cert`.
2. Read `target/cert/summary.html` for the human overview.
3. Inspect `target/cert/certificate.json` for the machine-readable evidence bundle, especially the formal-verification scope fields and per-proof proof-check results.
4. Re-check proof objects with `x07 prove check --proof <path>` when independent proof verification is required.
5. Use `x07 review diff --fail-on proof-coverage-decrease|assumption-surface-widen|bounded-proof-introduced|coverage-summary-imported|operational-entry-diverges|boundary-relaxation|trusted-subset-expansion` when comparing a baseline certificate posture to a candidate change.

For sandboxed or capsule-backed projects, add the posture gates below as needed:

- `async-proof-coverage-decrease`
- `capsule-contract-relaxation`
- `capsule-set-change`
- `sandbox-policy-widen`
- `runtime-attestation-regression`
- `weaker-isolation-enabled`

For the networked sandbox profile, add the network/package posture gates:

- `network-allowlist-widen`
- `peer-policy-relaxation`
- `capsule-network-surface-widen`
- `package-set-change`
- `lockfile-hash-change`
- `advisory-allowance-enabled`

The canonical example projects at `docs/examples/verified_core_pure_v1/`, `docs/examples/trusted_sandbox_program_v1/`, `docs/examples/trusted_network_service_v1/`, `docs/examples/certified_capsule_v1/`, and `docs/examples/certified_network_capsule_v1/` are structured to exercise those certificate flows end to end.
The checked-in `.github/workflows/certify.yml` files in those examples show the minimal CI surface for running the same flows in GitHub Actions, and `x07-mcp/docs/examples/verified_core_pure_auth_core_v1/` plus `x07-mcp/docs/examples/trusted_program_sandboxed_local_stdio_v1/` serve as the first-party package-backed dogfood examples in this train.

## CI artifact pattern

```bash
x07 review diff --from . --to . --html-out target/review/diff.html --json-out target/review/diff.json
x07 trust report --project x07.json --out target/trust/trust.json --html-out target/trust/trust.html
```

Upload `target/review/**` and `target/trust/**` as CI artifacts for reviewers.
