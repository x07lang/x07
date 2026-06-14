# verified_core_pure_v1

This example is the canonical smallest project that satisfies the `verified_core_pure_v1`
trust posture end-to-end. It is pure: no capsule boundary, no network, no
dependency-closure attestation.

Profile and entry for the [trust / verify cookbook](../trust-cookbook.md):

- `<profile>` — `arch/trust/profiles/verified_core_pure_v1.json`
- `<entry>` — `example.main`

The static posture step is just the profile check (there is no capsule index to
validate), and the test step runs the smoke + PBT suite.

The accepted certificate should report `entry_body_formally_proved: true` and a
non-empty operational-entry proof inventory.

CI:

- `.github/workflows/certify.yml` installs `x07`, `cbmc`, and `z3`, then uploads
  `target/cert/` as a review artifact.
