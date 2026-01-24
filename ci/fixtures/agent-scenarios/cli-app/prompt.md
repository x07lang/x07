# Scenario: cli-app (repair)

You are given a broken X07 project snapshot.

Goal:

- Make `x07 fmt`, `x07 lint`, `x07 fix`, `x07 run`, and `x07 test` succeed deterministically.
- Do not add new features; make the smallest repair that converges.

Expected workflow:

1. Run `x07 lint` and read the x07diag output.
2. Apply `x07 fix --write`.
3. Re-run `x07 fmt --check` and `x07 lint`.
4. Run `x07 run` and `x07 test`.

