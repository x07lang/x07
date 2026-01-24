# Scenario: data-cbor/missing-dep (self-repair)

Goal:

- The project fails to compile because it imports `ext.cbor.data_model` but is missing `ext-cbor-rs`.
- Make `x07 run` succeed deterministically.

