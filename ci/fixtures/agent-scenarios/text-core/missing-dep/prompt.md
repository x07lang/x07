# Scenario: text-core/missing-dep (self-repair)

You are given a broken X07 project snapshot.

Goal:

- The project fails to compile because it imports `std.text.ws` but is missing `ext-text`.
- Make `x07 run` succeed deterministically.

Expected workflow:

1. Run `x07 run` and read the compile error.
2. Apply the suggested fix: `x07 pkg add ext-text@0.1.1 --sync`.
3. Re-run `x07 run` and verify it succeeds.

