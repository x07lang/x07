# Benchmarks

Benchmark suites live here and are executed by `labs/scripts/bench/run_bench_suite.py` against committed reference solutions in `labs/benchmarks/solutions/`.

## Suites

- solve-pure sanity: `labs/benchmarks/solve-pure/phase3-suite.json`
- H1 (ABI/types baseline, solve-pure):
  - fast gate: `labs/benchmarks/solve-pure/phaseH1-smoke.json`
  - main suite: `labs/benchmarks/solve-pure/phaseH1-suite.json`
  - debug-only gate: `labs/benchmarks/solve-pure/phaseH1-debug-suite.json`
- H2 (stdlib parity across worlds):
  - `labs/benchmarks/solve-pure/phaseH2-suite.json`
  - `labs/benchmarks/solve-pure/phaseH2-collections-suite.json`
  - `labs/benchmarks/solve-fs/phaseH2-suite.json`
  - `labs/benchmarks/solve-rr/phaseH2-suite.json`
  - `labs/benchmarks/solve-kv/phaseH2-suite.json`
  - fast gate (solve-full): `labs/benchmarks/solve-full/phaseH2-smoke.json`
  - `labs/benchmarks/solve-full/phaseH2-suite.json`
- curriculum tiers (solve-pure): `labs/benchmarks/solve-pure/phase4-tier{1,2,3}-*.json`
- holdout (solve-pure): `labs/benchmarks/solve-pure/phase4-holdout.json`

Suite bundles (multi-suite runs):

- `labs/benchmarks/bundles/phaseH1.json`
- `labs/benchmarks/bundles/phaseH2.json`
- `labs/benchmarks/bundles/phaseH1H2.json`

Run the default bundle:

- `python3 labs/scripts/bench/run_bench_suite.py --suite labs/benchmarks/bundles/phaseH1H2.json`

H1/H2 suite smoke (native backend):

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
