# Trust / verify example cookbook

The `verified_core_pure_v1`, `certified_capsule_v1`, `certified_network_capsule_v1`,
`trusted_sandbox_program_v1`, and `trusted_network_service_v1` examples all carry a
trust profile end to end and share the same command flow. Each example README lists
only its profile path, entry symbol, and certificate expectations; this page holds
the steps they have in common.

Every command below uses placeholders the example README fills in:

- `<profile>` — the trust profile JSON under `arch/trust/profiles/`
- `<entry>` — the fully-qualified entry symbol to certify (for example `example.main` or `capsule.main`)

## Static posture checks

Validate the trust profile against the project and entry:

```bash
x07 trust profile check \
  --project x07.json \
  --profile <profile> \
  --entry <entry>
```

Capsule-backed examples also validate the capsule index plus referenced contracts
and attestations:

```bash
x07 trust capsule check \
  --project x07.json \
  --index arch/capsules/index.x07capsule.json
```

Networked examples additionally emit a dependency-closure attestation:

```bash
x07 pkg attest-closure \
  --project x07.json \
  --out target/dep-closure.attest.json
```

## Tests

Run the sandboxed tests on a host with a supported VM backend:

```bash
x07 test --all --manifest tests/tests.json
```

## Certificate bundle

Emit the certificate bundle for the entry:

```bash
x07 trust certify \
  --project x07.json \
  --profile <profile> \
  --entry <entry> \
  --out-dir target/cert
```

`target/cert/` is generated local output. Review `target/cert/summary.html` and
`target/cert/certificate.json`. Re-check the emitted proof object independently:

```bash
x07 prove check --proof target/cert/prove/<entry>/proof.json
```

## Re-emit the capsule attestation snapshot

Capsule-backed examples track a capsule attestation snapshot that can be re-emitted
from the contract + digest inputs:

```bash
x07 trust capsule attest \
  --contract arch/capsules/capsule.main.contract.json \
  --module src/capsule.x07.json \
  --lockfile x07.lock.json \
  --conformance-report arch/capsules/capsule.main.conformance.json \
  --out arch/capsules/capsule.main.attest.json
```

## CI

`.github/workflows/certify.yml` runs the same flow on a VM host: static posture
checks, any loopback service startup, sandboxed tests, and certificate emission.
