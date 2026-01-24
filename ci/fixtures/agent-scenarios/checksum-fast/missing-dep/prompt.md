# Scenario: checksum-fast/missing-dep (self-repair)

Goal:

- The project fails to compile because it imports `ext.checksum.crc32c` but is missing `ext-checksum-rs`.
- Make `x07 run` succeed deterministically.

