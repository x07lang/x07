# Stdlib ergonomics (parsing, bytes, JSON, path)

This guide covers small stdlib helpers that remove common boilerplate in CLI/data tools.

You can inspect signatures any time with:

```bash
x07 doc std.parse.u32_dec
x07 doc std.bytes.trim_ascii_view
x07 doc std.json.encode
x07 doc std.path.join_checked
```

## Canonical usage

### Parsing: decimal ints

- `std.parse.u32_dec(b)` returns `result_i32`:
  - `ok(x)` on success
  - `err(code)` on failure
- `std.parse.i32_dec(b)` returns `result_i32` with the same shape.

Parse error codes:

- `1`: empty / whitespace-only
- `2`: invalid form (non-decimal characters, missing digits, or sign in `u32_dec`)
- `3`: overflow

### Bytes: common view helpers

- `std.bytes.trim_ascii_view(b)` trims ASCII whitespace (`' '`, `'\t'`, `'\n'`, `'\r'`) and returns a `bytes_view`.
- `std.bytes.strip_prefix_view(a, prefix)` returns `a` without `prefix` when present.
- `std.bytes.strip_suffix_view(a, suffix)` returns `a` without `suffix` when present.

### JSON: convenience encoding

`std.json.encode(json, opts)` normalizes a JSON value (input is JSON text bytes; leading/trailing whitespace allowed):

- `opts & 1 != 0`: canonical key ordering (deterministic, compact)
- `opts & 1 == 0`: preserve object member order (trims leading/trailing whitespace)

`std.json.pretty_encode(json)` returns pretty-printed canonical JSON (2-space indent).

On invalid JSON, both return the same sentinel as `std.json.canonicalize_small`: `bytes.lit "ERR"`.

### Paths: safe-by-default helpers

- `std.path.normalize_posix(p)` normalizes `.` / `..` and collapses repeated slashes.
- `std.path.is_safe_relative(p)` returns `1` only if the normalized path is non-empty, not absolute, and contains no `..` segments.
- `std.path.parent(p)` returns the normalized parent path.
- `std.path.join_checked(out_root, rel)` returns `result_bytes` and rejects unsafe `rel` after normalization.

### Runnable example

Project: `docs/examples/agent-gate/stdlib-ergonomics/`

```bash
# Pass CLI-style args (encoded as argv_v1 after --):
x07 run --offline --repair=off --project docs/examples/agent-gate/stdlib-ergonomics/x07.json -- tool --max-entries 1000 --rel foo/../bar//baz

# Run the example tests (includes an unsafe-rel case):
X07_OFFLINE=1 x07 test --manifest docs/examples/agent-gate/stdlib-ergonomics/tests/tests.json
```

## Expert notes

- `std.parse.*_dec` trims ASCII space/tab (`0x20` / `0x09`) before parsing.
- `std.json.encode(json, opts)`: `opts & 1` selects canonical key ordering.
- `std.path.join_checked(out_root, rel)` returns `result_bytes.err(1)` when `rel` is unsafe.
