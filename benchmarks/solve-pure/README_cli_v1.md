# solve-pure CLI v1 determinism smoke

This suite is meant to be used as a **pure-world regression check** for `std.cli`:

- SpecRows JSON parsing/validation is deterministic.
- Compiled spec behavior is deterministic.
- Argv parsing is deterministic.

## Input encoding (per case)

`input` bytes are:

```
u32_le spec_len
spec_len bytes: UTF-8 JSON (x07cli.specrows@0.1.0)
remaining bytes: argv_blob_v1
```

### argv_blob_v1

```
u32_le argc
repeat argc:
  u32_le len
  len bytes: UTF-8 token bytes (no NUL terminator)
```

## Expected output encoding (matches_v1, success)

```
u8  tag = 1
u32_le cmd_len
cmd_len bytes cmd_utf8 (scope string, e.g. "root" or "root.sub")
u32_le entry_count
repeat entry_count (sorted by key bytes ascending):
  u32_le key_len
  key bytes (UTF-8)
  u8  kind: 1=flag, 2=opt, 3=arg
  u32_le value_len
  value bytes
```

For a present flag, value bytes are a single byte `0x01`.

## What this suite does NOT test

- OS argv capture
- help text rendering / wrapping
- exit codes

Those belong in `run-os*` suites.

