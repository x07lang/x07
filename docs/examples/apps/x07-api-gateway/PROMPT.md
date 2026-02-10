# Roadmap App 1 — Replayable API gateway (route + normalize + RR upstream)

```text
You are an X07 coding agent building `x07-api-gateway`.

GOAL
Build a small but production-shaped API gateway core that:
- normalizes request method/path/body deterministically (solve-pure)
- selects a route deterministically (solve-pure)
- can replay an upstream HTTP response deterministically (solve-rr via std.rr)
- emits review artifacts (lint/test/trust/review diff) suitable for CI

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart

REPO LAYOUT
- src/app/gateway/core.x07.json      (pure core: normalize/route/shape)
- src/app/gateway/os_main.x07.json   (thin adapter: handle_request_v1)
- src/app/gateway/upstream/rr.x07.json (solve-rr adapter: rr.next_v1 → bytes)
- tests/tests.json + tests/gateway_tests.x07.json
- arch/rr/* and arch/budgets/* pinned contracts
- ci/telemetry.sh + ci/trust.sh + ci/review_diff.sh

RR REQUIREMENTS
- Provide an rr policy under arch/rr/ that allows an HTTP GET op id.
- Include a committed cassette fixture under tests/fixtures/replay/rr/.
- Solve-rr tests must use std.rr.with_policy_v1 and assert the replayed response is non-empty.

TESTS
- solve-pure unit tests for normalize/path/header helpers and route selection.
- solve-rr replay test that reads the committed cassette and validates response bytes.
- 1 PBT test (bytes input) that checks a simple invariant (e.g. header canonicalization preserves length + is lowercase).

DELIVERABLES
- README.md with minimal run/test/bundle commands.
- All artifacts are deterministic and can be regenerated locally.
```

