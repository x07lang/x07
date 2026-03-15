# certified_capsule_v1

This example is the smallest standalone certified-capsule project for the
`certified_capsule_v1` trust posture.

It includes:

- `capsule.main` as the certifiable public boundary
- capsule contract + effect-log + attestation snapshot
- sandboxed smoke tests that record capsule evidence

Run the static posture checks:

```bash
x07 trust profile check \
  --project x07.json \
  --profile arch/trust/profiles/certified_capsule_v1.json \
  --entry capsule.main

x07 trust capsule check \
  --project x07.json \
  --index arch/capsules/index.x07capsule.json
```

Run the sandboxed tests on a host with a supported VM backend:

```bash
x07 test --all --manifest tests/tests.json
```

Emit a certificate bundle:

```bash
x07 trust certify \
  --project x07.json \
  --profile arch/trust/profiles/certified_capsule_v1.json \
  --entry capsule.main \
  --out-dir target/cert
```

Re-emit the tracked capsule attestation snapshot:

```bash
x07 trust capsule attest \
  --contract arch/capsules/capsule.main.contract.json \
  --module src/capsule.x07.json \
  --lockfile x07.lock.json \
  --conformance-report arch/capsules/capsule.main.conformance.json \
  --out arch/capsules/capsule.main.attest.json
```

`.github/workflows/certify.yml` is the minimal self-hosted VM runner workflow for
running the same capsule certification flow in CI.
