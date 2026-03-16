# trusted_sandbox_program_v1

This example is the smallest sandboxed trusted-program project that carries the
current sandboxed-local trust profile end to end:

- async `example.main` entry under `run-os-sandboxed`
- certified capsule boundary at `capsule.main`
- capsule contract + effect-log + attestation snapshot
- sandboxed `x07test` entries with runtime-attestation and capsule evidence

Run the static posture checks:

```bash
x07 trust profile check \
  --project x07.json \
  --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json \
  --entry example.main

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
  --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json \
  --entry example.main \
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
running the same sandboxed trust flow in CI.
