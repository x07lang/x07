# Roadmap App 3 — DB migration + drift guard (arch-pinned plan + RR verification)

```text
You are an X07 coding agent building `x07dbguard`.

GOAL
Build a small but production-shaped DB migration + drift guard tool that:
- keeps plan parsing/fingerprinting deterministic (solve-pure)
- applies migrations in OS worlds from repo-local arch/db/ contracts (run-os / run-os-sandboxed)
- verifies drift deterministically (solve-pure + solve-rr fixtures)

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart

DEPENDENCIES (lock them)
- ext-db-core + ext-db-sqlite (std.db.*)
- ext-db-migrate (std.db.migrate.*_from_arch_v1)
- ext-data-model (dsn doc helpers)
- ext-crypto-rs + ext-hex-rs (sha256 + hex encoding)
- ext-json-rs (canonical JSON)

REPO LAYOUT
- arch/db/index.x07db.json + arch/db/migrations/** (plan + sql + sha256)
- arch/rr/** (policies for solve-rr verification)
- arch/budgets/** (budget profiles for plan/apply/verify)
- src/app/plan.x07.json        (pure: canonicalize + fingerprint)
- src/app/fingerprint.x07.json (pure: sha256(plan) → hex)
- src/app/verify.x07.json      (pure: compare expected fingerprint)
- src/app/apply.x07.json       (os: std.db.migrate.apply_from_arch_v1)
- src/app/util.x07.json        (generics: stable_unique<A>)
- tests/tests.json with unit + PBT + RR replay tests
- policy/sandbox.base.json for run-os-sandboxed

CLI REQUIREMENTS
- stdin-driven commands:
  - `plan`
  - `verify`
  - `apply <sqlite_path>`

TESTS
- solve-pure unit tests for fingerprint determinism + stable_unique generic usage
- 1 PBT test for fingerprint determinism
- solve-rr tests that replay committed rr cassettes and validate verify output

DELIVERABLES
- README.md with run/test/bundle commands
- ci/telemetry.sh + ci/trust.sh + ci/review_diff.sh
```

