# x07 agent ergonomics — 2026-02-24..2026-03-01 (verification)

This note records which concrete issues from:

- `dev-docs/notes/x07-agent-ergonomics-2026-02-24.md`
- `dev-docs/notes/x07-agent-ergonomics-2026-02-25.md`
- `dev-docs/notes/x07-agent-ergonomics-2026-02-26.md`
- `dev-docs/notes/x07-agent-ergonomics-2026-02-27.md`
- `dev-docs/notes/x07-agent-ergonomics-2026-02-28.md`
- `dev-docs/notes/x07-agent-ergonomics-2026-03-01.md`

…are now enforced by the toolchain and/or covered by repo gates.

## CI gates run

- Toolchain gate: `cd x07 && ./scripts/ci/check_all.sh`
- MCP kit gate (local deps): `cd x07-mcp && PATH="../x07/target/debug:$PATH" X07_MCP_LOCAL_DEPS=1 ./scripts/ci/check_all.sh`

## Verified items

### `x07 bundle` fuel overrides

- `x07 bundle --solve-fuel <u64>` exists and is documented.
- The flag is wired through to the OS runner invocation for bundled executables (so the override applies at runtime, not only at compile time).

### Lockfile mismatch and hydration ergonomics

- Lock verification errors in `x07c` include a concrete hint command and add the `X07_WORKSPACE_ROOT=...` prefix when `$workspace/...` paths are present.
- `.x07/deps/...` lock entries always include `yanked` metadata (filled as `false` for vendored deps when index metadata is absent) so lock output is stable across “hydrated from registry” vs “vendored already present” paths.
- `x07 pkg lock --check` hydrates missing vendored deps under `.x07/deps/` (including patched-by-path vendored deps), so clean workspaces do not require a separate materialization bootstrap step.

### x07AST editing ergonomics

- `x07 fmt --pretty` exists, is deterministic, and is covered by toolchain tests.

### `x07 test` friction items

- “0 tests selected” is an error by default; `--allow-empty` opts into success for filter debugging loops.
- `solve_fuel` is supported per test entry in `x07.tests_manifest` and is validated (must be `>= 1`).
- Runner traps surface as `X07T_RUN_TRAP` with the decoded trap string on failures.
- `x07 test --verbose` emits per-test progress lines to stderr.

### Publishing / registry verification friction

- `x07 pkg publish` includes HTTP response bodies on error (GET/POST), so registry lint failures are actionable without re-packing + `curl`.
- `x07 pkg versions --refresh` exists to force cache-busted sparse-index reads when verifying freshly published versions.

### Runtime traps include path context for filesystem opens

- OS-world filesystem open failures now include attempted path context in trap strings (reduces “which file?” loops when debugging nested configs).

### `x07-mcp` signed PRM metadata edge case

- `ext-mcp-auth@0.4.5` verifies PRM signed metadata only when `signed_metadata` is present (missing `signed_metadata` returns `ok=true` without requiring secrets).
- `x07-mcp` package tests cover this via `auth/prm_signed/verify_missing_signed_metadata_ok` and the repo gate runs package tests for the full ext package matrix.

