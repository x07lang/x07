# XTAL example: toy-sorter

This example demonstrates a spec-first workflow:

- `spec/toy.sorter.x07spec.json` defines an operation and a PBT property (`ensures_props`).
- `spec/toy.sorter.x07spec.examples.jsonl` provides example cases.
- `gen/xtal/` contains deterministic generated tests derived from the spec.

## Quick check

Run the full wrapper:

- `x07 xtal verify --project x07.json`

Key outputs:

- Aggregate summary: `target/xtal/verify/summary.json`
- Per-entry verification reports: `target/xtal/verify/coverage/` and `target/xtal/verify/prove/`
- Wrapper diagnostics report: `target/xtal/xtal.verify.diag.json`

## Generated artifacts gate

This project uses the standard generator index at `arch/gen/index.x07gen.json`.

- Verify determinism + drift: `x07 gen verify --index arch/gen/index.x07gen.json`
- Regenerate outputs: `x07 gen write --index arch/gen/index.x07gen.json`

## Implementation conformance

- Check exports/signatures/contracts: `x07 xtal impl check --project x07.json`
- Sync exports/contracts/stubs from spec: `x07 xtal impl sync --project x07.json --write`
