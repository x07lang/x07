# Scenario: math-bigint/missing-dep (self-repair)

Goal:

- The project fails to compile because it imports `std.math.bigint.parse` but is missing `ext-bigint-rs`.
- Make `x07 run` succeed deterministically.

