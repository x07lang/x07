# Fixture formats (solve worlds)

This page documents the on-disk fixture formats used by deterministic worlds:

- `solve-fs`
- `solve-rr`
- `solve-kv`
- `solve-full`

The host runner copies fixture directories into a temporary run directory and enforces safe, deterministic path handling.

## Filesystem fixtures (solve-fs)

In `solve-fs`, the runner mounts a fixture directory as a read-only filesystem.

Conventions used by `x07 test`:

- If the fixture directory contains `root/`, that directory becomes the filesystem root (`.`).
- If the fixture directory contains `latency.json`, it is used for deterministic latency modeling.

### `latency.json` (`x07.fs.latency@0.1.0`)

```json
{
  "format": "x07.fs.latency@0.1.0",
  "default_ticks": 0,
  "paths": {
    "hello.txt": 5,
    "data/input.bin": 100
  }
}
```

Notes:

- `paths` keys are relative paths (use `/` separators).
- Values are latency in “ticks” applied by the deterministic scheduler.

The runner compiles this into a binary index at `.x07_fs/latency.evfslat` inside the run directory.

## Request/response fixtures (solve-rr)

In `solve-rr`, fixtures live under a `.x07_rr/` directory in the run directory.

The runner copies your fixture directory into `.x07_rr/` (read-only) and the runtime reads cassette files from there.

### Cassette files (`*.rrbin`)

A cassette file is a **u32-le framed stream**:

```
[u32_le length][length bytes payload][u32_le length][payload]...
```

Each payload is one RR `entry_v1` record encoded as a DataModel doc.

`entry_v1` is a DataModel ok-map with required keys:

- `kind` (bytes string): operation kind (for example `http`, `process`, `tcp_stream`, `file`, or `rr`)
- `op` (bytes string): stable operation id
- `key` (bytes string): match key (may be empty for transcript-mode cassettes)
- `req` (bytes string): request payload
- `resp` (bytes string): response payload
- `err` (number-as-bytes): `"0"` for ok; otherwise stable error code

Optional keys:

- `latency_ticks` (number-as-bytes): virtual-time ticks to sleep before returning the entry

Notes:

- Map keys must be sorted lexicographically (canonical encoding).
- Values are validated against RR budgets at load/append time.

### Recording fixtures with `x07 rr record`

`x07 rr record` appends a single `entry_v1` record to a cassette file under your fixture directory (creating it if missing).

## Key/value fixtures (solve-kv)

In `solve-kv`, fixtures live under a `.x07_kv/` directory in the run directory.

The runtime consumes binary files:

- `.x07_kv/seed.evkv` (seeded KV entries)
- `.x07_kv/latency.evkvlat` (latency per key + default)

You can provide these binaries directly, or provide a JSON seed file and let the runner compile it.

Conventions used by `x07 test`:

- If the fixture directory contains `seed.json`, it is used as the seed source.

### `seed.json` (`x07.kv.seed@0.1.0`)

```json
{
  "format": "x07.kv.seed@0.1.0",
  "default_latency_ticks": 0,
  "entries": [
    { "key_b64": "aGVsbG8=", "value_b64": "d29ybGQ=", "latency_ticks": 0 }
  ]
}
```

Notes:

- Keys and values are base64-encoded bytes.
- The runner sorts entries by key bytes to keep iteration and lookup deterministic.

## Combined fixtures (solve-full)

`solve-full` expects a single fixture root directory containing three subdirectories:

- `fs/` (filesystem fixtures)
- `rr/` (request/response fixtures)
- `kv/` (key/value fixtures)

Each subdirectory follows the same format rules as the corresponding single-world fixture.
