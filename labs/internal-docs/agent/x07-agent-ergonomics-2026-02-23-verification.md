# x07 agent ergonomics — 2026-02-23 (verification)

This note records the concrete issues reproduced while wiring a nested project under `x07-mcp/conformance/client-x07/` and the places they are enforced in the toolchain.

## 1) `..` segments rejected in `x07.json` paths

- `x07c` rejects `..` path segments in project path fields (`validate_rel_path`), so nested projects cannot reference sibling paths via `../../...`.

## 2) `.x07/deps/...` dependencies require index entries when online

- For dependencies whose `path` starts with `.x07/deps/`, `x07 pkg lock` consults the sparse index (when metadata is available) to fill yank/advisory metadata and fails when `{name,version}` has no index entry.

## 3) PKCE S256 helper was missing from a convenient surface

- No `std.auth.pkce` helper existed in `ext-auth-jwt` prior to adding `ext-auth-jwt@0.1.3`.

## 4) `application/x-www-form-urlencoded` encoder helper was missing

- No `std.net.http.form_urlencoded` helper existed in `ext-net` prior to adding `ext-net@0.1.9`.

## 5) `&` / `|` are eager (non-short-circuit)

- `&` / `|` evaluate both sides eagerly (bitwise i32 ops), so guard patterns like `(view.len v)` combined with `(view.get_u8 v 0)` can still trap.

## 6) Lint could miss a pattern that later failed during compile/bundle

- Returning a `bytes_view` that borrows from a local binding inside a statement block is invalid; the compiler/backend can reject it even when a surface lint did not flag the shape.

## 7) Runtime traps previously lacked an x07AST pointer

- Runtime trap messages are now expected to include an x07AST pointer (`ptr=/...`) when available, to converge repairs faster.

