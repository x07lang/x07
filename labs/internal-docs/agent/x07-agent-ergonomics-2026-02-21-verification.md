# Verification appendix: x07 agent ergonomics notes (2026-02-21)

Source note: `/Users/webik/projects/x07lang/dev-docs/notes/x07-agent-ergonomics-2026-02-21.md`

This appendix records whether each claim is **Verified / Not verifiable / Obsolete** (plus evidence pointers).

## Claims (2026-02-21)

### 1) Editing large one-line x07AST JSON files

Status: **Verified**

Evidence:
- Example large offender is a single very-long JSON line:
  - `/Users/webik/projects/x07lang/x07-mcp/packages/ext/x07-ext-mcp-transport-http/0.3.3/modules/ext/mcp/server.x07.json`
    - size: 238,482 bytes; `wc -l` reports `1`.
- Many shipped ext modules in `x07/packages/ext/` are also one-line JSON (often with no trailing newline):
  - `/Users/webik/projects/x07lang/x07/packages/ext/x07-ext-regex/0.1.0/modules/ext/regex.x07.json`
    - size: 113,231 bytes; `wc -l` reports `0` (no trailing newline).

### 2) `set` expression return type trips `if` branch typing

Status: **Verified**

Evidence:
- Typechecker returns the destination variable’s type from `set`:
  - `/Users/webik/projects/x07lang/x07/crates/x07c/src/typecheck.rs` (`fn infer_set`) returns `TyInfoTerm { ty: var.ty, ... }`.
- Minimal repro (fails with `X07-TYPE-IF-0002`):
  - `solve=["begin",["let","b",["bytes.lit","x"]],["if",1,["set","b",["bytes.lit","y"]],0],"b"]`
  - `x07 check --project …` reports `then=bytes` / `else=i32` at `ptr=/solve/2`.
- The intended ergonomic workaround (`set0`) typechecks:
  - `solve=["begin",["let","b",["bytes.lit","x"]],["if",1,["set0","b",["bytes.lit","y"]],0],"b"]`.

### 3) `bytes.view` on non-identifiers

Status: **Verified**

Evidence:
- Lint rule emits `X07-BORROW-0001` and explicitly requires an identifier owner:
  - `/Users/webik/projects/x07lang/x07/crates/x07c/src/lint.rs` (`X07-BORROW-0001` message: “requires an identifier owner (bind the value to a local with `let` first)”).
- Published language guide documents the same restriction:
  - `/Users/webik/projects/x07lang/x07/docs/spec/language-guide.md` (“Note: `bytes.view`, `bytes.subview`, and `vec_u8.as_view` require an identifier owner…”).

### 4) `x07 fmt` CLI ergonomics (`--input` required)

Status: **Obsolete (fixed)**

Evidence:
- `x07 fmt` now accepts positional paths in addition to `--input`:
  - `/Users/webik/projects/x07lang/x07/crates/x07/src/toolchain.rs`
  - `/Users/webik/projects/x07lang/x07/crates/x07/tests/cli.rs` (`x07_fmt_accepts_positional_paths`)

## Follow-up (2026-02-22)

### 1) “Alias for readability” can trigger `use after move`

Status: **Verified (with clarification)**

Evidence:
- Compiler has dedicated `use after move` errors carrying `moved_ptr=...`:
  - `/Users/webik/projects/x07lang/x07/crates/x07c/src/c_emit_builtins.rs` (`emit_ident_to`)
  - `/Users/webik/projects/x07lang/x07/crates/x07c/src/c_emit_async.rs` (multiple sites).
- The move-on-bind behavior applies to **owned** types (not view-like types):
  - `/Users/webik/projects/x07lang/x07/crates/x07c/src/c_emit.rs` (`is_owned_ty` includes `bytes`, `vec_u8`, etc.; `is_view_like_ty` covers `bytes_view` and does not move on `let`).
- In the cited function, the moved value is `bytes` (owned), not `bytes_view`:
  - `/Users/webik/projects/x07lang/x07-mcp/packages/ext/x07-ext-mcp-core/0.3.2/modules/std/mcp/logging.x07.json`
    - `logger_b` is produced by `option_bytes.unwrap_or` and used as `bytes` (e.g. `bytes.len logger_b`), so `let logger_v = logger_b` would move the owned buffer.

### 2) Gate failure for `external-packages.lock` drift is correct but opaque

Status: **Obsolete (fixed)**

Evidence:
- `x07-mcp` CI gate checks drift via `--check`:
  - `/Users/webik/projects/x07lang/x07-mcp/scripts/ci/check_all.sh` runs:
    - `python3 scripts/generate_external_packages_lock.py ... --check`
- The generator script’s `--check` failure message instructs regeneration:
  - `/Users/webik/projects/x07lang/x07-mcp/scripts/generate_external_packages_lock.py`
- The generator now supports `--write` (explicit write mode) in both repos:
  - `/Users/webik/projects/x07lang/x07/scripts/generate_external_packages_lock.py`
  - `/Users/webik/projects/x07lang/x07-mcp/scripts/generate_external_packages_lock.py`

### 3) Registry index caching can confuse “verify publish”

Status: **Not verifiable (requires real publish timing), but consistent with implementation/docs**

Evidence:
- `x07 pkg versions` reads from the sparse index client (not the registry API):
  - `/Users/webik/projects/x07lang/x07/crates/x07/src/pkg.rs` (`pkg_versions_report` uses `SparseIndexClient::fetch_entries`).
- Internal agent docs already warn about sparse-index caching and recommend API verification:
  - `/Users/webik/projects/x07lang/x07/AGENT.md` (“Sparse index reads are cached (~5 minutes); prefer verifying publishes via the registry API (`GET /v1/packages/<name>`).”).
- `x07 pkg publish` now performs a best-effort post-publish API fetch and warns (without failing) if the API does not reflect the new version yet:
  - `/Users/webik/projects/x07lang/x07/crates/x07/src/pkg.rs` (`cmd_pkg_publish`)
