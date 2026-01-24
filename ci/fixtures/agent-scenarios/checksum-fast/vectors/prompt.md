# Scenario: checksum-fast/vectors (golden vectors)

You are given an X07 project snapshot that depends on `ext-checksum-rs`.

Goal:

- Run the project tests deterministically and verify they pass offline.

Notes:

- Canonical dependency workflow is `x07 pkg add ext-checksum-rs@0.1.0 --sync`.
- This scenario uses the package-provided tests:
  - `ext.checksum.tests.test_crc32c_vectors`
  - `ext.checksum.tests.test_xxhash64_vectors`

