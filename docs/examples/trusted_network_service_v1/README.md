# trusted_network_service_v1

This example is the smallest sandboxed networked trusted-program project that
exercises the current sandboxed-network trust profile end to end:

- async `example.main` entry under `run-os-sandboxed`
- network capsule boundary at `capsule.main`
- pinned peer-policy file for the loopback TCP service
- effect-log + capsule attestation snapshot
- runtime-attested sandbox tests and dependency-closure evidence

Run the static posture checks:

```bash
x07 trust profile check \
  --project x07.json \
  --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json \
  --entry example.main

x07 trust capsule check \
  --project x07.json \
  --index arch/capsules/index.x07capsule.json

x07 pkg attest-closure \
  --project x07.json \
  --out target/dep-closure.attest.json
```

Start the loopback echo service that the capsule is pinned to:

```bash
python3 tests/tcp_echo_server.py --host 127.0.0.1 --port 30030
```

Run the sandboxed tests on a host with a supported VM backend:

```bash
x07 test --all --manifest tests/tests.json
```

Emit a certificate bundle:

```bash
x07 trust certify \
  --project x07.json \
  --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json \
  --entry example.main \
  --out-dir target/cert
```

Re-check the emitted proof object:

```bash
x07 prove check --proof target/cert/prove/example.main/proof.json
```

Review the accepted certificate for both halves of the trust story: the operational async entry should still report `entry_body_formally_proved: true`, while the network surface remains explicit through capsule attestation, peer-policy, dependency-closure, and runtime-attestation evidence.

Re-emit the tracked capsule attestation snapshot:

```bash
x07 trust capsule attest \
  --contract arch/capsules/capsule.main.contract.json \
  --module src/capsule.x07.json \
  --lockfile x07.lock.json \
  --conformance-report arch/capsules/capsule.main.conformance.json \
  --out arch/capsules/capsule.main.attest.json
```

`.github/workflows/certify.yml` runs the same flow in CI: static posture checks,
loopback service startup, sandboxed tests, and certificate emission on a VM host.
