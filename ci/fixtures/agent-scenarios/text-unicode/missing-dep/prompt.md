# Scenario: text-unicode/missing-dep (self-repair)

Goal:

- The project fails to compile because it imports `ext.unicode.normalize` but is missing `ext-unicode-rs`.
- Make `x07 run` succeed deterministically.

