# verified_core_pure_v1

This example is the canonical smallest project that satisfies the `verified_core_pure_v1`
trust posture end-to-end.

Run the profile check:

```bash
x07 trust profile check \
  --profile arch/trust/profiles/verified_core_pure_v1.json \
  --project x07.json \
  --entry example.main
```

Run the smoke + PBT suite:

```bash
x07 test --all --manifest tests/tests.json
```

Emit a certificate bundle:

```bash
x07 trust certify \
  --project x07.json \
  --profile arch/trust/profiles/verified_core_pure_v1.json \
  --entry example.main \
  --out-dir target/cert
```

A tracked reference certificate snapshot is kept at `target/cert/certificate.json`.
The rest of `target/cert/` remains generated local output.

CI example:

- `.github/workflows/certify.yml` installs `x07`, `cbmc`, and `z3`, then uploads `target/cert/` as a review artifact.
