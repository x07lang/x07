# argv_v1 fixtures

This directory contains deterministic fixtures for the `argv_v1` encoding used by CLI apps.

Encoding:

- `u32_le(argc)`
- then `argc` tokens of:
  - `u32_le(len)`
  - `len` raw bytes (UTF-8 for normal CLI usage)

`cases.json` stores both token arrays and the expected base64 of the encoded bytes.
