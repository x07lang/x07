# certified_network_capsule_v1

This example is the smallest standalone network capsule project that can be
reviewed under the networked sandbox certification line.

It includes:

- `capsule.main` as the certifiable public boundary
- pinned loopback peer-policy and network capsule contract
- effect-log + capsule attestation snapshot
- sandboxed smoke tests that record runtime-attestation and capsule evidence

Profile and entry for the [trust / verify cookbook](../trust-cookbook.md):

- `<profile>` — `arch/trust/profiles/trusted_program_sandboxed_net_v1.json`
- `<entry>` — `capsule.main`

This posture has both a capsule boundary and a network surface, so the cookbook's
static posture step runs the profile check, the capsule check, and the
dependency-closure attestation, and you re-emit the tracked capsule attestation
snapshot.

Before the sandboxed tests, start the loopback echo service that the capsule is
pinned to:

```bash
python3 tests/tcp_echo_server.py --host 127.0.0.1 --port 30030
```
