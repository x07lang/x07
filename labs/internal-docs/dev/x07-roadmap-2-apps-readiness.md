# Roadmap 2 apps — production readiness assessment

This note consolidates findings from the three “roadmap apps” reports and tracks what is verified vs. report noise.

Inputs:

- `docs/examples/apps/x07-api-gateway/report.md`
- `docs/examples/apps/x07crawl/report.md`
- `docs/examples/apps/x07dbguard/report.md`

## Verified issues (fixed)

### Tooling + workflows

- `x07 fmt` / `x07 lint` / `x07 fix` accept directory inputs and repeated `--input`, enabling simple CI gates over `src/` + `tests/`.

### `x07 arch check` recovery loop

- RR index sortable arrays (`kinds_allowed`, `ops_allowed`, `worlds_allowed`) now emit suggested JSON Patch fixes and can be applied via `x07 arch check --write`.
- RR sanitizer schema mismatch diagnostics include expected vs. got.
- RR sanitizer “schema-valid but missing required fields” emits a suggested patch adding defaults.

### RR fixture recording ergonomics

- `x07 rr record --cassette <path>` writes to the **exact safe relative path** passed (no extra prefixing).

### Generics + `ty.*` intrinsics monomorphization

- `ty.read_le_at` / `ty.write_le_at` / `ty.push_le` / `ty.hash32` now implicitly pull in the needed std modules so monomorphized heads do not fail at runtime.

### External package ecosystem breakage

- Added `ext-db-migrate@0.1.2` to remove a pinned broken transitive dependency (`ext-crypto-rs@0.1.0`) and updated the capability catalog accordingly.

## Verified issues (still open)

These are real friction points seen repeatedly across the reports and reference apps, but are not addressed yet.

### Docs + diagnostics

- Contract purity: clarify the “contract-pure” subset and expose a stable list of allowed heads.
- `bytes.view` borrow diagnostics: add actionable hints; expand `x07 fix` coverage for nested temporary borrows where safe.
- `bytes` move diagnostics: add a hint for `std.bytes.copy` on common “use after move” patterns.
- Assertion diagnostics: include “expected vs got” for byte/string assertions (at least as hex + utf8-lossy).

### Agent workflow gaps

- Project-level lint/check entry point (`x07 check` and/or `x07 lint --project x07.json`) to support a fast “lint → fix → lint” loop across multi-module projects without running full compilation.
- Structured JSON parse errors with x07AST context (ptr / decl path) for single-line minified files.

### Package manager resilience

- A principled transitive dependency override mechanism for known-broken versions (design work required; avoid ad-hoc overrides).

## Action plan (concrete next steps)

1. **Docs-only (low risk)**
   - Add a “contract-pure” section under `docs/language/` describing:
     - builtins vs `std.*` module calls in contracts,
     - the allowed operator/head set,
     - common rewrites (`std.bytes.len` → `bytes.len`, etc.).
   - Add an RR fixtures short section linking `fixture_root` + `.x07_rr/` resolution with examples.

2. **Diagnostics improvements (small toolchain change)**
   - Improve `X07-CONTRACT-0002` messaging to:
     - state “module calls are disallowed; only builtins/operators are contract-pure”
     - include 2–3 examples of allowed heads.
   - Improve `bytes.view requires identifier owner` diag with an explicit “bind then view” hint.

3. **Repair loop ergonomics (medium toolchain change)**
   - Extend `x07 fix` to safely rewrite nested temporary borrow patterns (`bytes.view(bytes.lit(..))`) into let-bound owners in more cases.
   - Add regression tests with minimal failing x07AST inputs.

4. **Project-level lint/check (medium/large)**
   - Design and implement a `--project x07.json` mode for `x07 lint` (or a new `x07 check`) that:
     - loads module roots + lockfile and resolves imports,
     - reports diagnostics across the full project.

Acceptance criteria for each item should be driven by the reference apps under `docs/examples/apps/` (add failing cases first, then fix).

