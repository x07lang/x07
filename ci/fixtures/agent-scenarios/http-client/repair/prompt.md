# Scenario: http-client (solve-rr repair)

You are given a broken X07 project snapshot intended to run deterministically in `solve-rr`.

Goal:

- Make `x07 fmt`, `x07 lint`, `x07 fix`, `x07 run`, and `x07 test` succeed deterministically.
- Do not add new features; make the smallest repair that converges.

Constraints:

- The project uses RR fixtures under `tests/fixtures/rr/`.
- The `solve-rr` world must not import OS-only modules.

