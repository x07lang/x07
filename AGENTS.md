# Repository Guidelines

## Project Status
This repository contains Track B scaffolding, a deterministic native runner, a compiler, and deterministic benchmark suites (H1/H2) that cover ABI/types plus stdlib parity across capability worlds.

## Project Structure & Module Organization
- `docs/`: end-user documentation (x07lang.org source)
- `crates/`: Rust workspace crates (compiler + runner)
- `benchmarks/`: benchmark suites + deterministic fixtures
- `scripts/bench/`: benchmark/curriculum tooling

## Build, Test, and Development Commands
- `cargo fmt --check`: verify formatting
- `cargo test`: run Rust unit/integration tests
- `cargo clippy --all-targets -- -D warnings`: lint Rust code
- `./scripts/ci/check_canaries.sh`: fast canary gate (tooling + smoke suites)
- `./scripts/ci/check_x07import_generated.sh`: verify x07import-generated stdlib modules are in sync
- `./scripts/ci/check_x07import_diagnostics_sync.sh`: verify `docs/x07import/diagnostics.md` matches the diagnostics catalog
- `./scripts/ci/check_suites_h1h2.sh`: execute H1/H2 suites on the native backend
- `./scripts/ci/check_asan_c_backend.sh`: run `x07-host-runner` tests with ASan/UBSan-enabled C artifacts
- `./scripts/ci/check_stdlib_lock.sh`: verify stdlib package manifests + lockfiles are in sync
- `python3 scripts/bench/generate_phase4_curriculum.py --check`: verify curriculum suites are up to date
- `python3 scripts/generate_stdlib_lock.py --check`: verify `stdlib.lock` matches `stdlib/std/**`
- `python3 scripts/bench/run_bench_suite.py --suite benchmarks/bundles/phaseH1H2.json`: run the default H1+H2 bundle against committed reference solutions
- `cargo test -p x07-host-runner`: run deterministic runner tests
- `cargo test -p x07`: run `x07 test` smoke/integration checks
- `cargo build -p x07c`: build the compiler
- `cargo run -p x07 -- test --manifest tests/tests.json`: run the built-in test harness smoke suite
- `cargo run -p x07-host-runner -- --program <program.x07.json> --world solve-pure --input <case.bin>`: compile→run `solve(bytes_view)->bytes`
- `cargo run -p x07c -- lock --project <project/x07.json>`: generate a project lockfile
- `cargo run -p x07c -- build --project <project/x07.json> --out <out.c>`: build a project to C
- `cargo run -p x07-host-runner -- --project <project/x07.json> --world solve-pure --input <case.bin>`: compile→run a project deterministically

## Canonical X07 surface

- Canonical solver source format: x07AST JSON (`*.x07.json`, `x07.x07ast@0.2.0`) with expressions encoded as JSON S-expressions (json-sexpr).
- Built-in stdlib version: `stdlib/std/0.1.1/`.
- Use `vec_u8.with_capacity` (not `vec_u8.new`) and `std.vec.as_bytes` (finalize a builder without copying).
- Prefer range builtins for performance: `vec_u8.extend_bytes_range` and `bytes.cmp_range`.
- Standalone-only systems surface (Phase H4): `unsafe` blocks, raw pointers, and `extern` C declarations/calls (world-gated; not available in `solve-*` worlds).
- For deterministic collection outputs, use `std.*.emit_*` (for example: `std.hash_set.emit_u32le`, `std.hash_map.emit_kv_u32le_u32le`, `std.heap_u32.emit_u32le`).
- For scan/trim/split without copying, prefer `bytes_view` + `view.*` builtins over copying helpers.
- For deterministic concurrency, use `defasync` + `task.*` + `chan.bytes.*` (no OS threads).
- In `run-os-sandboxed`, thread-backed blocking operations are gated by `policy.threads` (for example, `threads.max_blocking = 0` disables blocking operations).
- For streaming parsing, prefer `std.io` / `std.io.bufread` (`io.read`, `bufread.fill`/`consume`) and world adapters (`std.fs.open_read`, `std.rr.send`, `std.kv.get_stream`) which return `iface` readers.

## Coding Style & Naming Conventions
- Docs: keep changes small, use descriptive headings, and include concrete examples/paths where helpful.
- Rust: `rustfmt`-clean and clippy-clean; crates follow the `x07-*` naming pattern.
- Interfaces: keep the solver ABI stable (see `crates/x07c/include/x07_abi_v2.h`) and keep execution deterministic and resource-bounded.

## Testing Guidelines
Prefer deterministic tests: no real network, fixed seeds, and explicit resource limits. Keep tests close to the module they cover (e.g., `crates/x07-host-runner/tests/`).

## Commit & Pull Request Guidelines
Use a simple convention:
- Commits: `docs: …`, `feat: …`, `fix: …`, `refactor: …`
- PRs: describe intent, link the relevant issue, and include a short “Test Plan” (even for docs).

## Security & Configuration Tips
Treat generated programs as untrusted. Never commit secrets (LLM API keys, LiteLLM config); use environment variables or local `.env` files excluded from version control.
