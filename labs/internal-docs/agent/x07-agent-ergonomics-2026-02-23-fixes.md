# x07 agent ergonomics — 2026-02-23 (fixes)

This note summarizes the concrete fixes implemented for the ergonomics issues encountered when wiring MCP auth conformance.

## Workspace-root paths (`$workspace/...`)

- Added `$workspace/...` path token support in project path resolution.
- `$workspace/...` resolves relative to `X07_WORKSPACE_ROOT` and is guarded so it cannot escape the workspace root.
- `..` segments remain rejected.

## Local-only dependency semantics

- Dependencies whose `path` does **not** start with `.x07/deps/` are treated as local-only.
- Local-only deps must exist on disk (new diagnostic: `X07PKG_LOCAL_MISSING_DEP`).
- `.x07/deps/...` deps retain index/yank/advisory semantics when index metadata is available.

## Short-circuit boolean operators

- Added `&&` / `||` (short-circuit logical ops; result is `0`/`1`).
- `&` / `|` remain eager bitwise ops.
- Added lint warning `X07-BOOL-0001` to flag eager `&` / `|` in `if` conditions when trap-prone view ops are present.

## `bytes.view_lit` (bytes_view literals)

- Added `bytes.view_lit` builtin for `bytes_view` string literals without allocating an owned `bytes` just to take a view.
- Added lint error `X07-BORROW-0002` for `begin`/`unsafe` blocks that return a `bytes_view` borrowing from a local binding.
- Updated the `X07-BORROW-0001` quickfix to rewrite `["bytes.view",["bytes.lit","..."]]` into `["bytes.view_lit","..."]`.

## Runtime trap pointers

- Runtime trap messages now include an x07AST pointer suffix (`ptr=/...`) when available.

## Helper packages

- `ext-auth-jwt@0.1.3`: `std.auth.pkce.pkce_s256_challenge_v1(verifier_utf8: bytes_view) -> bytes`
- `ext-net@0.1.9`: `std.net.http.form_urlencoded.append_kv_v1(buf: vec_u8, k: bytes_view, v: bytes_view) -> vec_u8`

