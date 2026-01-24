# Scenario: data-msgpack/missing-dep (self-repair)

Goal:

- The project fails to compile because it imports `ext.msgpack.data_model` but is missing `ext-msgpack-rs`.
- Make `x07 run` succeed deterministically.

