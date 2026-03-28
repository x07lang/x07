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

Re-check the emitted proof object:

```bash
x07 prove check --proof target/cert/prove/example.main/proof.json
```

`target/cert/` is generated local output. Review `target/cert/summary.html` and `target/cert/certificate.json`; the accepted certificate should report `entry_body_formally_proved: true` and a non-empty operational-entry proof inventory.

CI example:

- `.github/workflows/certify.yml` installs `x07`, `cbmc`, and `z3`, then uploads `target/cert/` as a review artifact.
