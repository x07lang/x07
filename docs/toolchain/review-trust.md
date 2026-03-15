# Review & trust artifacts

`x07 review diff` and `x07 trust report` generate deterministic review artifacts for agent-authored changes.

For the smallest certificate-first example, see:

- `docs/examples/verified_core_pure_v1/`

For a second certifiable example built around published packages, see:

- `x07-mcp/docs/examples/verified_core_pure_auth_core_v1/`

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
  - `async-proof-coverage-decrease`
  - `boundary-relaxation`
  - `trusted-subset-expansion`
  - `capsule-contract-relaxation`
  - `capsule-set-change`
  - `sandbox-policy-widen`
  - `runtime-attestation-regression`
  - `weaker-isolation-enabled`

JSON schema:

- `spec/x07-review.diff.schema.json`
- `schema_version: "x07.review.diff@0.3.0"`

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
- arch manifest posture (`allowlist_mode`, cycles/orphans/visibility/world-caps),
- boundary index wiring for the certifiable trust surface,
- capsule requirements, runtime-attestation requirements, effect-log requirements, and sandbox backend/network posture when the selected profile asks for them.

Current built-in profiles include:

- `arch/trust/profiles/verified_core_pure_v1.json`
- `arch/trust/profiles/trusted_program_sandboxed_local_v1.json`
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

Use these commands to validate the capsule index plus referenced contracts/attestations, and to emit deterministic `x07.capsule.attest@0.1.0` artifacts that `x07 trust certify` can bind into a certificate bundle.

## Trust certify (`x07 trust certify`)

```bash
x07 trust certify \
  --project x07.json \
  --profile arch/trust/profiles/verified_core_pure_v1.json \
  --entry app.main \
  --out-dir target/cert
```

The certificate bundle includes:

- `certificate.json`
- `summary.html`
- `boundaries.report.json`
- `schema-derive/*.json` when boundary schemas are referenced
- `verify.coverage.json`
- per-symbol prove reports under `prove/`
- `tests.report.json`
- `trust.report.json`
- `compile.attest.json`
- capsule attestation references, runtime-attestation references, and effect-log digests when they are present in the observed evidence

`x07 trust certify` rejects if proof coverage regresses, required boundary metadata is missing, boundary-declared smoke/PBT tests do not resolve and pass, schema-derived outputs drift, trust report cleanliness fails, or compile attestation cannot bind the emitted native artifact.

Certificate schema:

- `spec/x07-trust.certificate.schema.json`
- `schema_version: "x07.trust.certificate@0.2.0"`

For `verified_core_pure_v1`, boundary-referenced schemas are rechecked with `x07 schema derive --check --out-dir .`, so certified projects should derive those schema outputs into the project root.

When a certified entry depends on reviewed imported helpers, `x07 verify --prove` uses the trusted primitive catalog in `catalog/verify_primitives.json` to model those calls in the proof harness. The coverage report still lists each trusted primitive explicitly so the certificate makes that trust boundary visible.

Current limitation:

- `trusted_program_sandboxed_local_v1` and other async/capsule-backed profiles can already validate profile posture and bundle runtime/capsule evidence, but accepted async certification is still blocked until `x07 verify` gains certifiable `defasync` proof and coverage support.

## Certificate-first review flow

For `verified_core_pure_v1`, the reviewer flow is:

1. Run `x07 trust certify ... --out-dir target/cert`.
2. Read `target/cert/summary.html` for the human overview.
3. Inspect `target/cert/certificate.json` for the machine-readable evidence bundle.
4. Use `x07 review diff --fail-on proof-coverage-decrease|boundary-relaxation|trusted-subset-expansion` when comparing a baseline certificate posture to a candidate change.

For sandboxed or capsule-backed projects, add the Milestone B posture gates as needed:

- `async-proof-coverage-decrease`
- `capsule-contract-relaxation`
- `capsule-set-change`
- `sandbox-policy-widen`
- `runtime-attestation-regression`
- `weaker-isolation-enabled`

The canonical example project at `docs/examples/verified_core_pure_v1/` is structured to exercise that flow end to end.
The checked-in `.github/workflows/certify.yml` file in that example shows the minimal CI surface for running the same certificate flow in GitHub Actions, and `x07-mcp/docs/examples/verified_core_pure_auth_core_v1/` serves as the second certifiable example project in this train.

## CI artifact pattern

```bash
x07 review diff --from . --to . --html-out target/review/diff.html --json-out target/review/diff.json
x07 trust report --project x07.json --out target/trust/trust.json --html-out target/trust/trust.html
```

Upload `target/review/**` and `target/trust/**` as CI artifacts for reviewers.
