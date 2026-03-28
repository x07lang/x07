# `x07 Atlas` Prompt

Implementation brief reproduced below.

```text
You are implementing a full-stack x07 WASM app to showcase the x07 ecosystem.

Goal: Build a production-grade demo app called “x07 Atlas”:
- Backend: WASI http/proxy component built from x07.
- Frontend: std.web_ui reducer built from x07, served by x07-wasm app serve.
- Core features:
  1) Offline-first UI (uses std.web_ui.effects.storage for caching).
  2) Multi-page navigation (uses std.web_ui.effects.nav).
  3) Backend API under /api:
     - GET /api/projects
     - POST /api/projects
     - GET /api/projects/{id}
     - POST /api/projects/{id}/items
     - Use JSON bodies; include stable error shapes.
  4) Deterministic E2E traces: create at least 3 traces in tests/:
     - happy path (create project, add items, refresh, see cache)
     - validation error path
     - performance/budget path (intentionally triggers fuel budget exceeded for one request)
  5) Incident → regression loop:
     - Make one test trace intentionally fail to produce a .x07-wasm/incidents/app/... bundle.
     - Then run x07-wasm app regress from-incident to generate tests/regress/*.trace.json and a final UI snapshot.
     - Ensure the generated regression is replayable with x07-wasm app test.

Repo layout (must follow):
- frontend/  (x07 project for web-ui reducer)
- backend/   (x07 project for http/proxy backend component)
- arch/      (profiles + ops/caps/policy/slo + wasm registries if needed)
- tests/     (trace files + goldens)
- dist/      (ignored by git)
- scripts/ci/check_showcase_fullstack.sh (self-contained gate script)

Hard requirements:
- Use x07-wasm app build/test/pack/verify/provenance attest/provenance verify/deploy plan/slo eval commands.
- Every command invocation MUST use: --json --report-out <file> --quiet-json.
- Never rely on external schema validators. Use x07-wasm *validate / *verify.
- Provide a single “demo pipeline” script scripts/ci/check_showcase_fullstack.sh that:
  1) builds
  2) tests (traces)
  3) packs
  4) verifies
  5) attests provenance
  6) verifies provenance
  7) emits a deploy plan
  8) runs slo eval on a fixture metrics snapshot (create a small fixture JSON)
- The script must exit nonzero on any failure.

Start from examples/app_fullstack_hello and examples/app_min as references, but implement the full Atlas feature set above.
```
