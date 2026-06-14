# trusted_network_service_v1

This example is the smallest sandboxed networked trusted-program project that
exercises the current sandboxed-network trust profile end to end:

- async `example.main` entry under `run-os-sandboxed`
- network capsule boundary at `capsule.main`
- pinned peer-policy file for the loopback TCP service
- effect-log + capsule attestation snapshot
- runtime-attested sandbox tests and dependency-closure evidence

Profile and entry for the [trust / verify cookbook](../trust-cookbook.md):

- `<profile>` — `arch/trust/profiles/trusted_program_sandboxed_net_v1.json`
- `<entry>` — `example.main`

This posture certifies the async operational entry over a network capsule
boundary, so the cookbook's static posture step runs the profile check, the
capsule check, and the dependency-closure attestation; you re-check the emitted
proof object and re-emit the tracked capsule attestation snapshot.

Before the sandboxed tests, start the loopback echo service that the capsule is
pinned to:

```bash
python3 tests/tcp_echo_server.py --host 127.0.0.1 --port 30030
```

Review the accepted certificate for both halves of the trust story: the
operational async entry should still report `entry_body_formally_proved: true`,
while the network surface remains explicit through capsule attestation,
peer-policy, dependency-closure, and runtime-attestation evidence.
