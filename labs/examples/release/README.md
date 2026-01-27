# Real-world release examples (cross-platform)

This directory contains larger x07 programs intended to exercise a wide surface area of the current stdlib + `packages/ext/*` integrations, and to make it easy to collect deterministic runtime metrics (`fuel_used`, `mem_stats`, `sched_stats`, etc.) via `x07-host-runner`.

## Capability snapshot (what’s currently practical)

- **Data + parsing (stdlib)**: `std.bytes`, `std.codec`, `std.parse`, `std.text.*`, `std.json` (small canonicalization + path extraction), `std.regex-lite` (literal search), `std.csv` (small helpers).
- **Data structures (stdlib)**: `std.{hash,btree}_{map,set}`, `std.small_{map,set}`, `std.lru_cache`, `std.heap_u32`, `std.deque_u32`, `std.slab`.
- **Deterministic I/O (solve-* worlds)**: `std.fs`, `std.rr`, `std.kv` via fixture-backed adapters (and async/task variants).
- **Common “real world” building blocks (ext packages)**: compression/archives (`ext.compress`, `ext.zip`, `ext.tar`), encoding (`ext.hex`, `ext.base64`), crypto (`ext.crypto`), structured formats (`ext.json.*`, `ext.toml.*`, `ext.yaml.*`, …).

## Included programs

- `artifact_audit/` (`solve-pure`): gzip → tar extraction + JSON pointer reads + SHA256/hex report.
- `zip_grep/` (`solve-pure`): list zip entries + extract text files + scan for a literal needle + SHA256/hex report.
- `full_pipeline/` (`solve-full`): fixture FS + RR + KV with async tasks; reads files, fetches per-file metadata, caches results, and emits a deterministic report.
- `web_crawler/` (`run-os`): YAML-configured web crawler using async tasks; crawls a site (same-origin), extracts links, and writes sorted URLs to a text file.

## Running (local)

Generate sample inputs (tar.gz + zip) once:

```bash
python3 labs/scripts/examples/generate_release_fixtures.py
```

Run the example harness (prints decoded output + key metrics):

```bash
python3 labs/scripts/examples/run_release_examples.py --all
```

## Running on Linux distros (Docker)

```bash
bash labs/scripts/examples/docker/run_matrix.sh
```

## Web crawler (run-os)

This example uses `x07-os-runner` (real OS filesystem + real network). The `x07.project@0.2.0` manifest keeps `world` deterministic (`solve-*`), so `web_crawler.x07project.json` is marked `solve-pure` and uses `default_profile: os` / `profiles.os.world: run-os` to make OS execution the default when using `x07 run`. If you invoke `x07-os-runner` directly, pass `--world run-os`.

Stage the ext-fs native backend once:

```bash
bash scripts/build_ext_fs.sh
```

Run via the helper script (prints decoded output + key metrics from the JSON report):

```bash
python3 labs/scripts/examples/run_web_crawler.py --config labs/examples/release/web_crawler/config.example.yaml
```

## Windows / WSL2

Use the same `python3 labs/scripts/examples/run_release_examples.py --all` command inside WSL2 (Ubuntu/Debian). For native Windows, build with MSVC or clang and run `cargo run -p x07-host-runner -- ...` (see `labs/scripts/examples/run_release_examples.py` for the exact flags).

For the web crawler example, prefer running inside WSL2 (same commands as Linux). Native Windows builds require a C toolchain plus `libcurl` headers/libs available on your toolchain’s include/lib search paths.
