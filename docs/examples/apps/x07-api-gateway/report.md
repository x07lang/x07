# x07 Agent Quickstart — Production Readiness Report

**Date**: 2026-02-09
**Toolchain**: x07 v0.1.10
**Agent**: Claude Opus 4.6 (via Claude Code CLI)
**Task**: Build a production-grade API gateway following the agent quickstart guide

---

## 1. Summary

Successfully built a complete x07 API gateway project:
- 4 source modules (pure core, OS adapter, RR adapter, app entry)
- 6 gateway functions with contracts
- Architecture manifests (budgets, RR policies, sanitizers)
- 12 tests (11 unit + 1 PBT with 100 cases)
- CI pipeline (3 scripts + GitHub Actions workflow)
- All tests pass, determinism gate passes, arch check passes, trust report generates

## 1.1 Verified status (post-report)

The original report was written against `x07 v0.1.10`. The following items were verified and are **now resolved in the toolchain**:

- Arch check schema mismatch diagnostics include **expected vs got** for `schema_version` mismatches.
- `x07 arch check` emits **suggested JSON Patch** quickfixes for:
  - sortable rr index arrays (`kinds_allowed`, `ops_allowed`, `worlds_allowed`)
  - sanitizer defaults when schema-valid but missing required fields.
- `x07 fmt` / `x07 lint` / `x07 fix` accept **directories and repeated `--input`**, so CI no longer needs per-file `find` loops.
- `x07 lint` can be run over a whole project directory (`--input src --input tests`) to avoid per-file cross-module false positives.

Items still open (real friction, not just report errors):

- Contract-purity and `bytes.view` borrow rules are correct but underdocumented; diagnostics could be more actionable.
- Assertion diagnostics in `std.test` still lack rich “expected vs got” printing for bytes.

## 2. What Slowed Me Down

### 2.1 Schema version strings are not discoverable
**Impact: HIGH** — Each arch file uses a unique schema_version string (e.g., `x07.arch.rr.sanitize@0.1.0` not `x07.arch.rr.sanitizer@0.1.0`). The difference between `sanitize` and `sanitizer` caused a hard error. I had to grep the toolchain's schema directory to find the correct string. The error message (`E_ARCH_RR_SANITIZER_SCHEMA_VERSION: schema_version mismatch`) does not say what the expected value is.

**Suggestion**: Error messages for schema_version mismatches should include the expected value: `expected "x07.arch.rr.sanitize@0.1.0", got "x07.arch.rr.sanitizer@0.1.0"`.

### 2.2 Sanitizer schema requirements not documented in guide/AGENT.md
**Impact: HIGH** — The sanitizer file requires `redact_headers` and `redact_token` fields but the guide only mentions creating a sanitizer without specifying the exact schema. I initially created `{"rules": []}` which is invalid. The error message was helpful (`"redact_headers" is a required property`) but required a trial-error loop.

**Suggestion**: The `x07 init` skeleton or AGENT.md should include a complete sanitizer example, or `x07 arch check` should offer `--write` auto-fix for trivially correctable schema errors.

### 2.3 Contract purity rules are non-obvious
**Impact: MEDIUM** — Contract `ensures` clauses must be pure. Functions like `std.bytes.len` and `std.bytes.get_u8` are NOT pure in this context — only builtins like `view.len` and operators are allowed. The error `X07-CONTRACT-0002: contract expression not pure` doesn't list which builtins ARE pure.

**Suggestion**: Either (a) the error should suggest a pure alternative, or (b) the `x07 guide` contract section should include a "pure builtins allowed in contracts" reference table.

### 2.4 `bytes.view` borrow rules require let-binding intermediates
**Impact: MEDIUM** — `["bytes.view", ["bytes.lit", "hello"]]` is invalid because `bytes.view` requires an identifier owner. The fix is to introduce a let binding: `["let", "x", ["bytes.lit", "hello"]], ["bytes.view", "x"]`. This pattern appears repeatedly. `x07 fix --write` auto-fixes most cases, but nested expressions inside function calls required manual intervention.

**Suggestion**: This is the most common error an agent will hit. Consider either (a) relaxing the rule to allow `bytes.view` on `bytes.lit` directly (since the literal is always alive), or (b) having `x07 fix --write` handle the nested case too.

### 2.5 `x07 fmt`/`lint`/`fix` only accept single files, not directories
**Impact: MEDIUM** — `x07 fmt --check src/` fails with "Is a directory". There is no glob or directory mode. For CI, I had to write shell loops with `find ... -print0 | while read`. This is fragile and agent-unfriendly.

**Suggestion**: Accept directory paths or glob patterns in `--input`. Most tools (rustfmt, prettier, eslint) accept directories.

### 2.6 `x07 lint` on individual files doesn't resolve cross-module references
**Impact: MEDIUM** — Running `x07 lint --input src/main.x07.json` produces false-positive `X07-TYPE-CALL-0001: unknown callee` errors because it doesn't see other modules. There is no project-level `x07 lint --project x07.json` command. The only way to get project-level type checking is through `x07 run` or `x07 test`.

**Suggestion**: Add `--project` mode to `x07 lint` so CI can do project-level lint without running.

### 2.7 PBT test configuration required multiple trial-error cycles
**Impact: LOW** — The PBT test manifest requires `params`, `returns: "bytes_status_v1"`, and `case_budget`, and the PBT function must accept `bytes` and return `bytes` (via `std.test.status_from_result_i32`). The guide mentions this but the relationship between manifest fields and function signatures took experimentation. The error `X07T_EPBT_PARAM_EMPTY` was clear but came after multiple other fixes.

**Suggestion**: The `x07 init` skeleton should include a complete PBT test example (both manifest entry and function).

### 2.8 `std.json.extract_path_canon_or_err` returns JSON-quoted strings
**Impact: LOW** — Extracting a string field from JSON returns `"POST"` (with quotes) not `POST`. This is correct canonical JSON behavior but surprised me and caused a test failure. The test assertion error (code 1003 = assert_bytes_eq) didn't show the actual vs expected values.

**Suggestion**: Test assertion failure messages should include hex dumps or string representations of actual vs expected values.

### 2.9 RR policy `kinds_allowed` must be sorted
**Impact: LOW** — `["rr", "http"]` fails with `E_ARCH_RR_POLICY_KINDS_NOT_SORTED`. Must be `["http", "rr"]`. This is a reasonable lint but the error could suggest the fix.

**Suggestion**: `x07 arch check --write` should auto-sort `kinds_allowed` arrays.

## 3. What Blocked Task Completion

**Nothing was a hard blocker.** Every error was eventually recoverable. The biggest risk of abandonment was the arch check loop (issues 2.1, 2.2, 2.9) which required 3 fix-retry cycles — an agent without access to schema files might have given up.

## 4. What Prevented Self-Recovery

### 4.1 No `--write` auto-fix for arch manifests
The `x07 arch check` command has `--write` and `--emit-patch` flags but produced `"suggested_patches": []` for all errors encountered. The sanitizer schema and kinds_allowed sorting could have been auto-fixed.

### 4.2 `x07 fix --write` doesn't handle all borrow violations
The auto-fixer handles simple cases but doesn't fix `bytes.view` nested inside function call arguments. Manual code restructuring was required.

### 4.3 Error messages lack "did you mean?" suggestions
When a function name or schema version is close but wrong, the toolchain doesn't suggest corrections. For an agent, a "did you mean X?" hint is the difference between immediate fix and a search through documentation.

## 5. Suggestions for Improving x07 for Coding Agents

### Priority 1: Improve error messages
1. Include expected values in schema_version mismatch errors
2. Show actual vs expected in test assertion failures
3. Add "did you mean?" for close-match function names and schema strings
4. List pure builtins in contract purity errors

### Priority 2: Expand auto-fix coverage
1. `x07 arch check --write` should fix sortable arrays and fill required fields with defaults
2. `x07 fix --write` should handle nested borrow violations (let-binding extraction)
3. `x07 fmt` / `x07 lint` should accept directories, not just individual files

### Priority 3: Better scaffolding
1. `x07 init` should generate a complete arch skeleton with valid sanitizer files
2. `x07 init` should generate a PBT test example (manifest + function)
3. AGENT.md should include complete file examples for every arch file type

### Priority 4: Project-level tools
1. Add `x07 lint --project x07.json` for project-level type checking
2. Add `x07 check` as a unified pre-commit gate (fmt + lint + arch + test)

## 6. Project Structure

```
test1/
├── x07.json                         # project manifest (21 deps)
├── x07.lock.json                    # lockfile
├── src/
│   ├── main.x07.json                # entry point
│   ├── app.x07.json                 # app module
│   └── app/gateway/
│       ├── core.x07.json            # pure core (6 functions, 2 with contracts)
│       ├── os_main.x07.json         # OS adapter (2 functions)
│       └── upstream/
│           └── rr.x07.json          # RR adapter (2 functions)
├── tests/
│   ├── tests.json                   # test manifest (12 entries)
│   ├── gateway_tests.x07.json       # test module (11 unit + 1 PBT)
│   └── smoke.x07.json               # init-generated smoke test
├── arch/
│   ├── manifest.x07arch.json        # architecture manifest
│   ├── manifest.lock.json           # arch lockfile (generated)
│   ├── contracts.lock.json          # contracts lockfile (generated)
│   ├── budgets/
│   │   ├── index.x07budgets.json    # budget index (2 profiles)
│   │   └── profiles/
│   │       ├── gw_request_v1.budget.json
│   │       └── gw_upstream_v1.budget.json
│   └── rr/
│       ├── index.x07rr.json         # RR index (1 policy)
│       ├── policies/
│       │   └── gw_rr_v1.policy.json
│       └── sanitizers/
│           └── sanitize_none_v1.sanitize.json
├── ci/
│   ├── telemetry.sh                 # full CI pipeline
│   ├── trust.sh                     # trust report generation
│   └── review_diff.sh               # semantic diff generation
└── .github/workflows/
    └── ci.yml                       # GitHub Actions workflow
```

## 7. Verification Results

| Check                        | Status |
|------------------------------|--------|
| `x07 run`                   | PASS   |
| `x07 test` (11 unit)        | PASS   |
| `x07 test --repeat 2`       | PASS   |
| `x07 test --pbt` (100 cases)| PASS   |
| `x07 arch check`            | PASS   |
| `x07 trust report`          | PASS   |
| `x07 fmt --check` (all)     | PASS   |
| `ci/telemetry.sh`           | PASS   |
