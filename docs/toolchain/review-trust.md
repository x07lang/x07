# Review & trust artifacts

`x07 review diff` and `x07 trust report` generate deterministic review artifacts for agent-authored changes.

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

JSON schema:

- `spec/x07-review.diff.schema.json`
- `schema_version: "x07.review.diff@0.1.0"`

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

## CI artifact pattern

```bash
x07 review diff --from . --to . --html-out target/review/diff.html --json-out target/review/diff.json
x07 trust report --project x07.json --out target/trust/trust.json --html-out target/trust/trust.html
```

Upload `target/review/**` and `target/trust/**` as CI artifacts for reviewers.
