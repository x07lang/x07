# Benchmarks

Benchmark suites live here and are executed by `labs/scripts/bench/run_bench_suite.py` against committed reference solutions in `labs/benchmarks/solutions/`.

## Suites

- solve-pure sanity: `labs/benchmarks/solve-pure/phase3-suite.json`
- ABI/types baseline (solve-pure):
  - fast gate: `labs/benchmarks/solve-pure/abi-types-smoke.json`
  - main suite: `labs/benchmarks/solve-pure/abi-types-suite.json`
  - debug-only gate: `labs/benchmarks/solve-pure/abi-types-debug-suite.json`
- Stdlib parity across worlds:
  - `labs/benchmarks/solve-pure/stdlib-parity-suite.json`
  - `labs/benchmarks/solve-pure/stdlib-parity-collections-suite.json`
  - `labs/benchmarks/solve-fs/stdlib-parity-suite.json`
  - `labs/benchmarks/solve-rr/stdlib-parity-suite.json`
  - `labs/benchmarks/solve-kv/stdlib-parity-suite.json`
  - fast gate (solve-full): `labs/benchmarks/solve-full/stdlib-parity-smoke.json`
  - `labs/benchmarks/solve-full/stdlib-parity-suite.json`
- curriculum tiers (solve-pure): `labs/benchmarks/solve-pure/phase4-tier{1,2,3}-*.json`
- holdout (solve-pure): `labs/benchmarks/solve-pure/phase4-holdout.json`

Suite bundles (multi-suite runs):

- `labs/benchmarks/bundles/abi-types.json`
- `labs/benchmarks/bundles/stdlib-parity.json`
- `labs/benchmarks/bundles/regression.json`

Run the default bundle:

- `python3 labs/scripts/bench/run_bench_suite.py --suite labs/benchmarks/bundles/regression.json`

Suite smoke (native backend):

- `./labs/scripts/ci/check_suites_h1h2.sh`

## Phase 4 curriculum generation

- Generate: `python3 labs/scripts/bench/generate_phase4_curriculum.py`
- Check up to date: `python3 labs/scripts/bench/generate_phase4_curriculum.py --check`

## Fixtures

Filesystem fixture snapshots live under:

`ci/fixtures/bench/fs/solve-fs/<suite_id>/root/`

Build a stable, read-only snapshot (normalized mtimes + deterministic manifest):

`./scripts/build_fixture_fs_tree.sh <fixture_id> <source_dir>`

Request/response fixtures live under:

- `ci/fixtures/bench/rr/solve-rr/<suite_id>/index.json` + `ci/fixtures/bench/rr/solve-rr/<suite_id>/bodies/**`.

Key/value fixtures live under:

- `ci/fixtures/bench/kv/solve-kv/<suite_id>/seed.json`

Solve-pure fixture blobs (for large inputs/outputs) live under:

- `ci/fixtures/bench/pure/<world>/<suite_id>/`

## Properties

Notes on holdouts and anti-overfit checks: `labs/benchmarks/properties/README.md`
