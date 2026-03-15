# certified_network_capsule_v1

This example is the smallest standalone network capsule project that can be
reviewed under the networked sandbox certification line.

It includes:

- `capsule.main` as the certifiable public boundary
- pinned loopback peer-policy and network capsule contract
- effect-log + capsule attestation snapshot
- sandboxed smoke tests that record runtime-attestation and capsule evidence

Run the static posture checks:

```bash
x07 trust profile check \
  --project x07.json \
  --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json \
  --entry capsule.main

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

`.github/workflows/certify.yml` runs the same flow in CI on a VM host.
