# trusted_sandbox_program_v1

This example is the smallest sandboxed trusted-program project that carries the
current sandboxed-local trust profile end to end:

- async `example.main` entry under `run-os-sandboxed`
- certified capsule boundary at `capsule.main`
- capsule contract + effect-log + attestation snapshot
- sandboxed `x07test` entries with runtime-attestation and capsule evidence

Profile and entry for the [trust / verify cookbook](../trust-cookbook.md):

- `<profile>` — `arch/trust/profiles/trusted_program_sandboxed_local_v1.json`
- `<entry>` — `example.main`

This posture certifies the async operational entry over a capsule boundary, with
no network surface, so the cookbook's static posture step runs the profile check
and the capsule check, you re-check the emitted proof object, and you re-emit the
tracked capsule attestation snapshot. There is no dependency-closure attestation.

The accepted certificate is still a capsule-backed trust story, but it should also
report that the operational async entry was formally proved:
`entry_body_formally_proved: true` and `formal_verification_scope` at least
`entry_body`.
