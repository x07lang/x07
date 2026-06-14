# certified_capsule_v1

This example is the smallest standalone certified-capsule project for the
`certified_capsule_v1` trust posture.

It includes:

- `capsule.main` as the certifiable public boundary
- capsule contract + effect-log + attestation snapshot
- sandboxed smoke tests that record capsule evidence

Profile and entry for the [trust / verify cookbook](../trust-cookbook.md):

- `<profile>` — `arch/trust/profiles/certified_capsule_v1.json`
- `<entry>` — `capsule.main`

This posture has a capsule boundary but no network surface, so the cookbook's
static posture step runs both the profile check and the capsule check, and you
re-emit the tracked capsule attestation snapshot. There is no dependency-closure
attestation and no separate proof-object re-check.
