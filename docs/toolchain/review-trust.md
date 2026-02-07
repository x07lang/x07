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
- SBOM placeholder inventory.

Optional observed inputs:

- `--run-report <path>`
- `--bundle-report <path>`
- `--x07test <path>`

Strict/fail gates:

- `--strict`
- `--fail-on allow-unsafe|allow-ffi|net-enabled|process-enabled|nondeterminism|sbom-missing`

JSON schema:

- `spec/x07-trust.report.schema.json`
- `schema_version: "x07.trust.report@0.1.0"`

## CI artifact pattern

```bash
x07 review diff --from . --to . --html-out target/review/diff.html --json-out target/review/diff.json
x07 trust report --project x07.json --out target/trust/trust.json --html-out target/trust/trust.html
```

Upload `target/review/**` and `target/trust/**` as CI artifacts for reviewers.
