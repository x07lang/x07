# `web_crawler/` (run-os)

OS-world example program that:

- reads a YAML config (passed as `--input` bytes),
- (YAML subset) parses a flat `key: value` mapping with optional `"` quotes and `#` comments (no nesting / lists / multiline scalars),
- crawls a website (same-origin),
- extracts `href="..."` / `href='...'` links (basic HTML scan),
- writes a sorted unique URL list to `output_path`,
- uses async tasks (`defasync`, `task.spawn`, `await`) and returns a per-page log (bounded) + summary.

## Config schema

See `examples/release/web_crawler/config.example.yaml` for a runnable template.

Required:
- `base_url` (string) — `http://...` or `https://...`
- `output_path` (string) — uses `/` separators (Windows too); relative to `examples/release/` when run via `--project`.

Optional:
- `max_depth` (int, default `1`)
- `max_pages` (int, default `200`)
- `max_concurrency` (int, default `8`)
- `timeout_s` (int, default `10`)
- `max_redirects` (int, default `5`)
- `max_body_bytes` (int, default `262144`)
- `max_links_per_page` (int, default `200`)

## Run (macOS / Linux / WSL2)

Prereqs (HTTP backend):

- `libcurl` headers + library (`ext-curl-c` includes a small C shim that links against libcurl)
  - macOS: Xcode Command Line Tools usually suffice (`xcode-select --install`); Homebrew `curl` also works.
  - Ubuntu/Debian/WSL2: `sudo apt-get install build-essential pkg-config libcurl4-openssl-dev`

Stage the ext-fs native backend once:

```bash
bash scripts/build_ext_fs.sh
```

Run via the helper script (prints decoded output + runner metrics):

```bash
python3 scripts/examples/run_web_crawler.py --config examples/release/web_crawler/config.example.yaml
```

Manual runner invocation:

```bash
cargo run -p x07-os-runner -- \
  --project examples/release/web_crawler.x07project.json \
  --world run-os \
  --input examples/release/web_crawler/config.example.yaml \
  --auto-ffi
```

Output:

- With the default config, the URL list is written to `examples/release/target/web_crawler/urls.txt` (because `output_path` is relative to `examples/release/`).

## Windows

Recommended: run it inside WSL2 (Ubuntu) using the same steps as above.

If you want native Windows (not WSL2), you need:

- Rust toolchain (`rustup`)
- A C toolchain and libcurl development files available to the linker (e.g. MSYS2 + `pacman -S mingw-w64-x86_64-toolchain mingw-w64-x86_64-curl`, or vcpkg + MSVC)
- Stage ext-fs native backend into `deps/x07/` (the helper script currently expects `scripts/build_ext_fs.sh`, which is bash-centric)

Note: `web_crawler.x07project.json` is marked `world: solve-pure` because the current `x07.project@0.2.0` schema is restricted to deterministic `solve-*` worlds, but this example must be run with `--world run-os`.
