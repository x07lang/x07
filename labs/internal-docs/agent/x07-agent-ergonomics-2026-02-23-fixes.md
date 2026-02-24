# x07 agent ergonomics — 2026-02-23 (status + fixes)

This note summarizes what we verified from `dev-docs/notes/x07-agent-ergonomics-2026-02-23.md` and the concrete fixes we applied.

## Workspace-root paths (`$workspace/...`)

- `$workspace/...` is supported in project path fields and is guarded so it cannot escape the workspace root.
- `$workspace/...` resolves relative to `X07_WORKSPACE_ROOT` (when set).
- Fix: when `X07_WORKSPACE_ROOT` is not set, tooling now infers the workspace root from the nearest git repository root (walking ancestors for `.git`).
- `..` segments remain rejected in manifest paths.

## Local-only dependency semantics

- Dependencies whose `path` does **not** start with `.x07/deps/` are treated as local-only (in-repo/vendored/unpublished deps).
- Local-only deps must exist on disk (`X07PKG_LOCAL_MISSING_DEP` when missing).
- `.x07/deps/...` deps are treated as registry-vendored and require an index match when index metadata is being consulted (yanks/advisories, etc.).

Practical guidance: for unpublished in-repo packages, use a local-only `path` (often via `$workspace/...`), not `.x07/deps/...`.

## Short-circuit boolean operators

- `&&` / `||` are short-circuit logical ops (use these for guard patterns that would otherwise trap).
- `&` / `|` are eager bitwise ops.
- Lint warning `X07-BOOL-0001` flags some trap-prone patterns involving eager `&` / `|` in `if` conditions.

## `bytes.view_lit` (bytes_view literals)

- `bytes.view` requires an identifier owner (it cannot borrow from a temporary); bind first with `let`.
- `bytes.view_lit` exists for literal `bytes_view` construction and tooling has quickfixes that rewrite `bytes.view(bytes.lit ...)` into `bytes.view_lit` when safe.
- Fix: `bytes.view_lit` now accepts whitespace in the literal argument (same as `bytes.lit`).

## Runtime trap pointers

- Runtime trap messages include an x07AST pointer suffix (`ptr=/...`) when available (verified via `view.slice oob` trap output).

## Helper packages (already available)

- `ext-auth-jwt@0.1.3`: `std.auth.pkce.pkce_s256_challenge_v1(verifier_utf8: bytes_view) -> bytes`
- `ext-net@0.1.9`: `std.net.http.form_urlencoded.append_kv_v1(buf: vec_u8, k: bytes_view, v: bytes_view) -> vec_u8`
