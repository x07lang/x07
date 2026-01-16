# X07

X07 is a deterministic compiler + runner for x07AST JSON programs (`solve(bytes_view)->bytes`). The language surface is defined by the compiler's built-in core semantics plus built-in stdlib modules.

This repository currently contains:

- scaffolding: interfaces, schemas, and project structure
- deterministic native runner (`crates/x07-host-runner`)
- standalone OS runner (`crates/x07-os-runner`, for `run-os*` worlds)
- compiler (`crates/x07c`)
- deterministic test harness (`crates/x07`, `x07 test`)
- committed benchmark suites + fixtures (H1/H2)
- committed reference solutions (`benchmarks/solutions/`)
- curriculum suites (optional; see `benchmarks/solve-pure/phase4-*.json`)

## Repository map (x07lang org)

- `x07lang/x07` — toolchain + stdlib + canonical docs (this repo)
- `x07lang/x07-website` — x07lang.org site (built from released docs bundles)
- `x07lang/x07-index` — package sparse index metadata
- `x07lang/x07-registry` — package registry server
- `x07lang/x07-perf-compare` — optional perf comparison harnesses (split out to keep `x07` lean)

## Downloads (official builds)

- Latest release: https://github.com/x07lang/x07/releases/latest
- All releases: https://github.com/x07lang/x07/releases

Each release includes `x07`, `x07c`, `x07-host-runner`, `x07-os-runner`, and `x07import-cli`.

Artifacts:
- macOS: `x07-<tag>-macOS.tar.gz`
- Linux: `x07-<tag>-Linux.tar.gz`
- Windows: `x07-<tag>-Windows.zip`
- Skills pack: `x07-skills-<tag>.tar.gz`
- Release manifest: `release-manifest.json` (see `docs/releases.md` and `docs/official-builds.md`)

## Repository layout

- `docs/`: end-user docs (x07lang.org source)
- `crates/`: Rust workspace (compiler + deterministic runner)
- `benchmarks/`: benchmark suites + fixtures
- `scripts/bench/`: benchmark/curriculum tooling

## LLM-first contracts

- Canonical solver source format: x07AST JSON (`*.x07.json`, `x07.x07ast@0.1.0`) with expressions encoded as json-sexpr (`["head", ...]`).
- Agent tooling surface (stable machine I/O): `x07c fmt`, `x07c lint`, `x07c fix`, `x07c apply-patch` (RFC 6902 JSON Patch).
- Built-in deterministic test harness: `x07 test` (manifest-driven; emits `x07test` JSON).
- Standalone-only systems surface (Phase H4): `unsafe` blocks, raw pointers, and `extern` C declarations/calls (world-gated; not available in `solve-*` worlds).

## Quick start (dev)

Prereqs:
- Rust toolchain (`cargo`)
- C compiler available as `cc` (override via `X07_CC`)
- `clang` (required for `x07import c` and C-import tests)
- Python 3 (stdlib only; used by `scripts/bench/` and a few repo maintenance scripts)

Rust workspace checks:
- `./scripts/ci/check_all.sh` (canonical full gate)
- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `./scripts/ci/check_x07import_generated.sh`
- `./scripts/ci/check_x07import_diagnostics_sync.sh`
- `./scripts/ci/check_suites_h1h2.sh`
- `./scripts/ci/check_asan_c_backend.sh` (C backend sanitizer gate)
- `cargo run -p x07 -- test --manifest tests/tests.json` (test harness smoke suite)

Build + run `solve-pure`:
- `cargo build -p x07-host-runner`
- `cargo run -p x07-host-runner -- --program <program.x07.json> --world solve-pure --input <case.bin>`

Example `program.x07.json` (echo):
- `{"schema_version":"x07.x07ast@0.1.0","kind":"entry","module_id":"main","imports":[],"decls":[],"solve":["view.to_bytes","input"]}`

Build + run `run-os` (standalone-only, not used by benchmark suites):
- `cargo build -p x07-os-runner`
- `cargo run -p x07-os-runner -- --program examples/h3/read_file_by_stdin.x07.json --world run-os --input <case.bin>`

Project example (multi-module + lockfile):
- `cargo run -p x07c -- lock --project examples/phaseE/x07.json`
- `cargo run -p x07-host-runner -- --project examples/phaseE/x07.json --world solve-pure --input <case.bin>`

Benchmark suites (H1/H2):
- `./scripts/ci/check_suites_h1h2.sh`
- `python3 scripts/bench/run_bench_suite.py --suite benchmarks/bundles/phaseH1H2.json`
- H2 collections suite (solve-pure): `benchmarks/solve-pure/phaseH2-collections-suite.json` (included in `benchmarks/bundles/phaseH2.json` and `benchmarks/bundles/phaseH1H2.json`)
- Stdlib emitters canary (solve-pure): `benchmarks/solve-pure/emitters-v1-suite.json` (included in `benchmarks/bundles/phaseH2.json` and `benchmarks/bundles/phaseH1H2.json`)

Curriculum suites (optional):
- `python3 scripts/bench/generate_phase4_curriculum.py --check`

## License

Licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT license (`LICENSE-MIT`)
