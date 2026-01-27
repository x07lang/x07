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

The runner requires an RR index and will accept either:

- `index.evrr` (prebuilt binary index), or
- `index.json` (source index; the runner will compile it to `index.evrr`).

### `index.json` (`x07.rr.fixture_index@0.1.0`)

```json
{
  "format": "x07.rr.fixture_index@0.1.0",
  "default_latency_ticks": 0,
  "requests": {
    "example.com": { "latency_ticks": 0, "body_file": "bodies/9c56cc51.bin" }
  }
}
```

Notes:

- `requests` maps a key (bytes in `std.rr.fetch(key)` / `std.rr.send(key)`) to a response.
- `body_file` is a safe relative path under `.x07_rr/` (for example `bodies/<...>.bin`).
- Additional fields may appear (for example `status`) and are ignored by the runner.

### Recording fixtures with `x07 rr record`

`x07 rr record --out fixtures/rr <key> <url>` writes:

- `fixtures/rr/index.json`
- `fixtures/rr/bodies/<sha256(key)>.bin` (raw response body bytes)

### `rr.send_request` response blobs

`std.rr.send_request(req_bytes)` uses a direct sha256 mapping (it does not consult `index.json`):

- `.x07_rr/responses/<sha256(req_bytes)>.bin`

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

