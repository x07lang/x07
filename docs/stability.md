# Stability policy

X07 prioritizes determinism and stable machine-readable contracts.

## ABI and contracts

- The solver interface is `solve(bytes_view)->bytes`; the canonical program format is defined by `spec/x07ast.schema.json`.
- JSON schemas under `spec/` define the canonical tool contracts (`x07diag`, `x07patch`, `x07test`).

Any breaking change to the solver ABI or canonical schemas requires an RFC and a versioned migration path.
