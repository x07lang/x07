# Repair corpus (golden quickfix regressions)

This fixture set is a deterministic regression suite for the agent repair loop:

- `x07 lint` emits machine-readable diagnostics (`x07c.report@0.1.0` with `x07diag` entries).
- Diagnostics include deterministic `quickfix` JSON Patch operations when possible.
- `x07 fix` applies the patches deterministically.

The gate asserts:

- expected diagnostic codes are present
- a JSON Patch quickfix exists (for quickfix cases)
- `x07 fix` + `x07 fmt --check` produce byte-stable output matching the golden `fixed.x07.json`

Do not assert on human text messages here; only stable codes and deterministic outputs.
