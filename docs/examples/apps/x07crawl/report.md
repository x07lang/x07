# x07 Agent Quickstart: Production Readiness Report

## Executive Summary

Successfully implemented the **x07crawl** tool following the agent quickstart guide.
All 12 tests pass: 8 unit (run-os), 1 PBT (solve-pure), 3 RR replay (solve-rr).
Trust gates pass. CI pipeline defined.

**Overall assessment**: The platform is usable by a code agent, but several friction
points caused significant slowdowns and required multiple retry cycles.

---

## 0. Verified status (post-report)

Some items in the original report were verified against the toolchain and current stdlib and turned out to be **report errors**:

- `std.hash_set` **is implemented** in the shipped stdlib; `"unsupported head: std.hash_set.view_new"` typically indicates the module was not loaded into the program (missing import), not that the API is absent.
- x07 has an error-code explainer: `x07 diag explain <CODE>` (so “no `--explain` command” is not accurate).

Items verified as real issues (and fixed in the toolchain):

- RR cassette path resolution for `solve-rr` is now documented (see `docs/worlds/record-replay.md`).
- `x07 rr record --cassette <path>` now writes to the **exact safe relative path** passed on the CLI (no `fixtures/rr/` prefixing).
- `x07 fmt` / `x07 lint` / `x07 fix` accept **directory inputs and repeated `--input`**, so CI no longer needs per-file loops.

## 1. What Slowed Me Down

### 1.1 Move semantics for `bytes` type (High Impact)

The `bytes` type is move-only. After passing a `bytes` value to a function
(e.g., `std.test.assert_bytes_eq`), the value is consumed and cannot be reused.
This is not obvious from the function signatures in `x07 doc` output, which only
show `bytes_view` parameter types without indicating consumption semantics.

**Time cost**: ~3 retry cycles to diagnose "use after move" errors in tests.
Required inserting `std.bytes.copy` calls before each consuming use.

**Suggestion**: Emit a diagnostic like `"hint: value 'x' was moved here; use
std.bytes.copy to retain a copy"` alongside the "use after move" error.

### 1.2 `bytes.view` requires identifier owner (Medium Impact)

`["bytes.view", ["bytes.lit", "..."]]` fails because `bytes.view` cannot borrow
from a temporary expression. Every `bytes.lit` must first be bound to a `let`
variable, then viewed separately.

**Time cost**: ~2 retry cycles. The error message `"bytes.view requires an
identifier owner"` is technically correct but doesn't explain why or show the fix.

**Suggestion**: The error could include a quickfix suggestion:
`"hint: bind the literal first: [\"let\", \"x\", [\"bytes.lit\", ...]] then
[\"bytes.view\", \"x\"]"`.

### 1.3 Contract purity restrictions (Medium Impact)

Contracts (`requires`/`ensures`) can only use built-in operators
(`view.len`, comparison ops). Calling any library function (even `std.bytes.eq`)
in a contract clause produces `X07-CONTRACT-0002: contract expression is not pure`.

**Time cost**: ~2 retry cycles. I wrote contracts calling `std.bytes.eq` and
`std.crawl.urlnorm.canon_v1`, which are logically pure but not "contract-pure".

**Suggestion**: The guide should define "contract-pure" explicitly and list which
built-in operators are allowed. Currently the guide mentions contracts but doesn't
enumerate the allowed expression subset.

### 1.4 Undocumented/unsupported APIs in the guide (High Impact)

The built-in language guide (`x07 guide`) documents `std.hash_set.view_new` and
`std.hash_set.view_insert`, but the compiler rejects them with
`"unsupported head: std.hash_set.view_new"`.

**Time cost**: ~2 retry cycles. Had to replace the hash-set-based dedup approach
with a linear-scan approach (matching the reference web-crawler-local example).

**Suggestion**: Either implement `std.hash_set` or remove it from the guide.
If APIs are experimental/planned, mark them as such.

### 1.5 RR cassette path resolution (Medium Impact)

When creating solve-rr tests, the cassette path in `std.rr.with_policy_v1` is
resolved relative to the `fixture_root` specified in the test manifest.
This isn't documented anywhere. I initially used full paths like
`"tests/fixtures/replay/rr/crawl_v1.rrbin"` and got failure code 2002 (cassette
not found). Only after changing to just `"crawl_v1.rrbin"` with `fixture_root`
pointing to the directory did it work.

**Suggestion**: Document the path resolution rule for cassettes in solve-rr world.
Also document the `fixture_root` field in the test manifest schema.

### 1.6 `x07 rr record` output directory (Low Impact)

`x07 rr record --cassette tests/fixtures/replay/rr/crawl_v1.rrbin` wrote the
file to `fixtures/rr/tests/fixtures/replay/rr/crawl_v1.rrbin` (prepending
`fixtures/rr/` to the `--cassette` path). Had to manually move files.

**Suggestion**: Make `--cassette` path behave as an exact output path, or
document the `out_dir` prefix behavior.

### 1.7 JSON-as-source format (Low-Medium Impact)

Writing x07AST as JSON arrays is verbose and error-prone. A single misplaced
bracket or comma breaks the entire module. Standard code editors provide no
syntax help (no bracket matching for semantic units, no jump-to-definition).

**Observation**: This is inherent to the JSON-as-IR design. For an agent, the
structured format is actually easier to manipulate programmatically than text
syntax. But debugging malformed JSON is harder than debugging text syntax errors.

---

## 2. What Blocked Task Completion

### 2.1 No hard blockers

All required features worked after finding the correct patterns. The quickstart
prompt was fully implementable.

### 2.2 Near-blockers

- **`std.hash_set` not implemented**: Would have been a hard blocker if the
  reference example hadn't shown a linear-scan workaround.
- **RR cassette creation**: If network access had been unavailable, creating
  `.rrbin` fixtures would have been blocked. There is no way to create synthetic
  cassettes without `x07 rr record` (which makes real HTTP requests).

---

## 3. What Prevented Self-Recovery

### 3.1 Error messages lack actionable fixes

Most x07 error messages state *what* is wrong but not *how* to fix it:

| Error | Says | Should Also Say |
|-------|------|-----------------|
| `use after move: x` | The value was consumed | `hint: use std.bytes.copy before the consuming call` |
| `bytes.view requires identifier owner` | Can't view a temporary | `hint: bind to a let variable first` |
| `CONTRACT-0002: not pure` | Expression is not contract-pure | `hint: only built-in ops allowed in contracts` |
| `unsupported head: std.hash_set.view_new` | Head not recognized | `hint: this API is not yet implemented` |

### 3.2 No `--explain` command

Languages like Rust have `rustc --explain E0382` which shows a full explanation
with examples for each error code. x07 error codes (CONTRACT-0002, etc.) have
no corresponding explain command.

**Suggestion**: Add `x07 diag --explain CONTRACT-0002` that shows the full
explanation, the allowed subset, and a working example.

### 3.3 Guide doesn't distinguish "contract-pure" from "pure"

The guide uses both terms without defining the difference. I assumed
`std.bytes.eq` (a pure function with no side effects) would be allowed in
contracts, but "contract-pure" is a much stricter subset (only built-in ops).

### 3.4 Lack of solve-rr documentation

The solve-rr world, `fixture_root` manifest field, and cassette path resolution
are undocumented or only documented by implication. I had to discover the correct
configuration through trial and error (3 attempts: missing fixture_root,
wrong path format, then correct).

---

## 4. Suggestions to Improve x07 for Coding Agents

### 4.1 Actionable error messages (Priority: High)

Every error should include a `hint:` line with the recommended fix pattern.
Code agents parse error messages to determine next actions - vague errors
cause retry loops.

### 4.2 Error catalog with examples (Priority: High)

Implement `x07 diag --explain <CODE>` showing:
- What the error means
- What triggers it
- A minimal working example
- A minimal broken example

### 4.3 Machine-readable diagnostics (Priority: High)

`x07 lint --json` and `x07 fix --json` already exist and work well.
Extend this to all error outputs: compile errors, test failures, and runtime
errors should all have `--json` structured output with error codes, source
locations, and suggested fixes.

### 4.4 Document the allowed contract expression subset (Priority: Medium)

Add a section to the guide: "Contract expressions may only use: comparison
operators (`=`, `!=`, `<`, `>`, `<=`, `>=`), arithmetic operators (`+`, `-`,
`*`), `view.len`, `bytes.view`, and literal values. Library function calls
are not allowed."

### 4.5 Remove or mark unimplemented APIs (Priority: Medium)

`std.hash_set.view_new` in the guide but not in the compiler wastes agent
cycles. Either implement it or add a `[NOT YET IMPLEMENTED]` marker.

### 4.6 Document solve-rr test setup (Priority: Medium)

Add a guide section covering:
- The `fixture_root` manifest field
- How cassette paths resolve relative to fixture_root
- The `x07 rr record` command and its output path behavior
- A complete solve-rr test example

### 4.7 Provide a quickfix for `bytes.view` of temporaries (Priority: Low)

`x07 fix` could auto-extract `["bytes.view", ["bytes.lit", "x"]]` into
`["let", "_tmp", ["bytes.lit", "x"]], ["bytes.view", "_tmp"]`.

### 4.8 Provide cassette creation without network (Priority: Low)

A synthetic cassette builder (`x07 rr create --key URL --body FILE --cassette OUT`)
would enable fully offline test setup without requiring HTTP access.

---

## 5. Summary Statistics

| Metric | Value |
|--------|-------|
| Source modules | 7 |
| Test modules | 2 |
| Unit tests | 8 (all pass) |
| PBT properties | 1 (200 cases, all pass) |
| solve-rr tests | 3 (all pass) |
| Total tests | 12 |
| RR cassette fixtures | 3 |
| Architecture files | 7 |
| CI scripts | 3 |
| Trust gates | All pass |
| Retry cycles (errors hit) | ~12 total |
| Distinct error categories | 6 |
| Hard blockers | 0 |
