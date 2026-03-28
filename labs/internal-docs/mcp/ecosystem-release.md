# MCP Kit ecosystem release checklist

This note tracks the MCP kit conformance + registry + packaging release policy.

## Pins

- MCP protocol version: `2025-11-25`
- Registry schema URL: `https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json`
- Conformance runner: `@modelcontextprotocol/conformance@0.1.14`
- MCPB CLI: `@anthropic-ai/mcpb@2.1.2`

## Conformance baseline policy

- Keep expected-failure baseline minimal.
- Current baseline keys: _none_ (`server: []`, `client: []`).
- CI fails on regressions and also fails when baseline entries become stale.

## Phase 4 verification checks

- `POST /mcp` SSE flow:
  - prime event emitted first
  - progress notification emitted when `_meta.progressToken` is requested
  - final JSON-RPC response event emitted unless cancelled
- `GET /mcp` listen SSE flow:
  - subscription updates routed to listen stream only
  - no-broadcast routing between request/listen streams
- Resumption:
  - `Last-Event-ID` resumes from bounded stream buffer
  - replay does not cross stream keys
- Cancellation:
  - `notifications/cancelled` stops in-flight request
  - cancelled request produces no final response
- Origin enforcement:
  - invalid Origin on POST or GET returns `403`

## Phase 6 verification checks

- Task progress invariants:
  - task-augmented `tools/call` progress token remains valid for task lifetime
  - progress notifications stop after the task reaches a terminal state
  - progress notifications include related-task metadata
- Deterministic RR guardrails:
  - `hello_tasks_progress` replay fixture is enforced in CI
  - replay fails if progress continues after terminal

## Phase 7 verification checks

- Logging semantics:
  - `logging/setLevel` clamps + rejects invalid params deterministically
  - `notifications/message` emission is redacted, rate-limited, and replay-stable
- Observability outputs:
  - audit sink emits deterministic JSONL sidecars when enabled
  - metrics snapshot/export wiring is exercised in kit tests (when enabled)

## Phase 8 verification checks

- PRM (RFC9728):
  - insertion URL is served (derived from configured `oauth.resource`)
  - root alias is served only when `serve_root_alias=true`
  - response is `200` with `Content-Type: application/json` and `resource` matches config
- OAuth RS enforcement on `POST /mcp`:
  - `?access_token=...` is rejected with `400` (empty body)
  - missing Authorization returns `401` + `WWW-Authenticate` (resource metadata URL + scope)
  - insufficient scope returns `403` + `WWW-Authenticate ... error=\"insufficient_scope\"`
- Strict Streamable HTTP headers:
  - invalid Origin returns `403`
  - invalid/missing Accept returns `400`
  - invalid protocol version returns `400`
  - HTTP-level failures have empty bodies (status + headers only)
- RR sanitizer boundary:
  - redacts Authorization / Proxy-Authorization / Cookie / Set-Cookie
  - fails closed if token-like patterns survive sanitization

## Phase 12 verification checks

- Trust framework (`x07.mcp.trust.framework@0.1.0`):
  - resource matching precedence is deterministic (`exact` > `prefix` > `hostSuffix` > defaults)
  - issuer allowlist and pinned key lookup resolve across composed bundles
- Publish dry-run trust gates:
  - `publish.require_signed_prm=true` rejects missing `signed_metadata`
  - signer issuer must be allowed for the resolved resource policy
  - signer key must be present in trust bundle pins
  - generated `_meta` trust summary matches publish inputs
- Release metadata guard:
  - reject placeholder `trustFrameworkSha256` on release tags
  - require `publisher-provided.x07.requireSignedPrm=true` in publisher metadata

## Phase 13 verification checks

- Trust framework v2 (`x07.mcp.trust.framework@0.2.0`) + lock (`x07.mcp.trust.lock@0.1.0`):
  - bundle publisher pins (`bundle_publishers`) are required for signed bundles
  - trust lock digests match canonical bundle + signature bytes
- Signed trust bundle statements:
  - `*.trust_bundle.sig.jwt` verifies for pinned issuer + `kid`
  - accepted `alg` allowlist and `iat/exp` windows are enforced
  - `bundle_sha256` claim must match canonical trust bundle SHA-256
- Governed multi-AS selection:
  - `authorization_servers` selection follows policy `prefer_order_v1` deterministically
  - fail-closed mode rejects PRM when no allowed issuer is present
  - conformance scenario `prm-multi-as-select-prefer-order` is green in CI
- Release metadata guard:
  - reject placeholder `trustLockSha256` on release tags
  - require trust lock + referenced signature files to exist for published server artifacts

## Phase 14 verification checks

- Trust framework v3 (`x07.mcp.trust.framework@0.3.0`) + lock v2 (`x07.mcp.trust.lock@0.2.0`):
  - `source.kind=url` / `sig_source.kind=url` is allowed only with matching lock entries
  - lock entries pin URL + digest pairs (`bundle_url`, `sig_url`, `bundle_sha256`, `sig_sha256`)
  - no-TOFU is enforced: remote bundle sources fail when lock pins are missing
- Trust pack metadata:
  - publish summary includes `x07.trustPack.{registry,packId,packVersion,lockSha256}` when configured
  - release guards reject missing/placeholder `packVersion` or `lockSha256`
- Template replay fixtures:
  - trust registry/pack install fixtures are present and wired in template tests
  - remote trust replay fixture validates 200/304 fetch path with deterministic cassette input

## Phase 15 verification checks

- TUF-lite registry metadata:
  - root/timestamp/snapshot metadata fixtures are present and parse/validate in package tests
  - timestamp/snapshot rollback is rejected when prior trusted versions are higher
  - expired timestamp/snapshot metadata is rejected
  - fast-forward jumps beyond policy cap are rejected
- Trust-pack anti-rollback metadata:
  - publish summary includes `trustPack.minSnapshotVersion`, `snapshotSha256`, `checkpointSha256`
  - release guards reject missing/zero/placeholder anti-rollback trust-pack fields
  - manifests with trust-pack metadata must provide a trust root file (`root_path`) that exists
- Template replay fixtures:
  - `tests/replay/trust.tuf_ok/http.jsonl` and `trust.tuf_rollback_timestamp/http.jsonl` are present and enforced in template tests

## Phase 16 verification checks

- Transparency tlog primitives:
  - Merkle root, inclusion proof, and consistency proof checks pass for the deterministic phase-16 dataset
  - checkpoint payload verification enforces expected `log_id`/`origin` and root/tree-size fields
- Monitor behavior:
  - monitor succeeds for expected append-only growth + allowed entries (`publish16/trust_tlog_monitor_ok`)
  - monitor rejects unexpected entries with policy violation (`publish16/trust_tlog_monitor_unexpected`)
  - monitor rejects inconsistent append-only proofs (`publish16/trust_tlog_monitor_inconsistent`)
- Fixture/replay coverage:
  - template dataset `templates/trust-registry-tlog/` is present and referenced by RR fixtures
  - `rr/http/trust_tlog_monitor_{ok,unexpected,inconsistent}.http.jsonl` sessions are present
  - conformance wrapper `conformance/trust-tlog/run.sh` is green in CI local-deps mode

## Gaps-2 verification checks

- Templates/scaffold:
  - no runtime secrets/private JWKs are committed under `templates/**/config/auth/`
  - scaffold generates unique runtime secrets/keys and `.gitignore` ignores them
- Auth SSRF:
  - `.test` is not treated as a safe allowlist
  - DNS failures fail-closed for SSRF checks
- Auth cache bounds:
  - OAuth decision cache is bounded by `auth.oauth_cache.*` and cannot grow unbounded
- HTTP request limits:
  - `transports.http.streamable.max_header_bytes` oversized => `431`
  - `transports.http.streamable.max_body_bytes` oversized => `413`
  - `transports.http.streamable.max_concurrent_requests` caps global request concurrency
- Sandbox tool spawn control:
  - `sandbox.router_exec.max_concurrent_per_tool` is enforced
  - `sandbox.router_exec.warm_pool_size_per_tool` optionally keeps idle workers
- Server config validation:
  - `std.mcp.toolkit.server_cfg_file` rejects unknown keys and type mismatches for both `x07.mcp.server_config@0.3.0` and legacy `@0.2.0`
- Perf smoke:
  - run `X07_MCP_PERF_SMOKE=1 ./scripts/ci/check_all.sh` (or the scheduled `perf-smoke` workflow) to catch process leaks/regressions

## Release checklist

1. Run `x07-mcp` checks (`./scripts/ci/check_all.sh` and reference server suites).
2. Build deterministic `.mcpb` artifact(s) and verify stable SHA-256 across repeat builds.
3. Generate `server.json` from `x07.mcp.json` and validate schema + non-schema constraints.
4. Run `x07 mcp publish --dry-run` for release artifacts.
5. Run tag-only release metadata guards (`release_metadata_guard.sh`, `release_guard_trust_lock_and_sig.sh`, `release_guard_server_json_mcpb_sha.sh`) to enforce non-placeholder trust + hash metadata.
6. Confirm docs and pin tables are synchronized across `x07-mcp` and `x07`.
