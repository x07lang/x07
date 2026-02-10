# Roadmap App 2 — Deterministic crawler planning + extraction (RR-first dataset builder)

```text
You are an X07 coding agent building `x07crawl`.

GOAL
Build a crawler-shaped project that proves the RR workflow:
- keep crawl planning + parsing pure and deterministic (solve-pure)
- record HTTP responses once into rr cassettes
- replay offline (solve-rr) to validate extraction output deterministically

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart

FUNCTIONAL REQUIREMENTS
- Planning module:
  - normalize URLs deterministically
  - enforce max-pages and stable dedup
  - expose a pure function that turns newline-separated seeds into a deterministic plan
- Extraction module:
  - extract at least one structured record from HTML
  - emit canonical JSON bytes
- CLI wrapper:
  - parse `--mode record|replay`, `--seeds`, `--out` (ext.cli specrows)
  - emit canonical JSON summary bytes

RR REQUIREMENTS
- Provide rr policy under arch/rr/.
- Commit rr cassettes under tests/fixtures/replay/rr/.
- Solve-rr tests must:
  - open cassette via std.rr.with_policy_v1
  - call std.rr.next_v1 and validate response bytes
  - run extraction against replayed HTML

TESTS
- unit tests for URL normalization, planning, and extraction
- 1 PBT test for an idempotence property (e.g. url normalize(normalize(x)) == normalize(x))
- 3 solve-rr tests that replay committed cassettes

DELIVERABLES
- README.md with record + test commands
- ci/telemetry.sh + ci/trust.sh + ci/review_diff.sh
```

