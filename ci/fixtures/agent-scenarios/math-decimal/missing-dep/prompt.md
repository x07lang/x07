# Scenario: math-decimal/missing-dep (self-repair)

Goal:

- The project fails to compile because it imports `std.math.decimal.parse` but is missing `ext-decimal-rs`.
- Make `x07 run` succeed deterministically.

