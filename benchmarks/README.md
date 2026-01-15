# Benchmarks

Benchmark suites live here and are executed by `scripts/bench/run_bench_suite.py` against committed reference solutions in `benchmarks/solutions/`.

## Suites

- solve-pure sanity: `benchmarks/solve-pure/phase3-suite.json`
- H1 (ABI/types baseline, solve-pure):
  - fast gate: `benchmarks/solve-pure/phaseH1-smoke.json`
  - main suite: `benchmarks/solve-pure/phaseH1-suite.json`
  - debug-only gate: `benchmarks/solve-pure/phaseH1-debug-suite.json`
- H2 (stdlib parity across worlds):
  - `benchmarks/solve-pure/phaseH2-suite.json`
  - `benchmarks/solve-pure/phaseH2-collections-suite.json`
  - `benchmarks/solve-fs/phaseH2-suite.json`
  - `benchmarks/solve-rr/phaseH2-suite.json`
  - `benchmarks/solve-kv/phaseH2-suite.json`
  - fast gate (solve-full): `benchmarks/solve-full/phaseH2-smoke.json`
  - `benchmarks/solve-full/phaseH2-suite.json`
- curriculum tiers (solve-pure): `benchmarks/solve-pure/phase4-tier{1,2,3}-*.json`
- holdout (solve-pure): `benchmarks/solve-pure/phase4-holdout.json`

Suite bundles (multi-suite runs):

- `benchmarks/bundles/phaseH1.json`
- `benchmarks/bundles/phaseH2.json`
- `benchmarks/bundles/phaseH1H2.json`

Run the default bundle:

- `python3 scripts/bench/run_bench_suite.py --suite benchmarks/bundles/phaseH1H2.json`

H1/H2 suite smoke (native backend):

- `./scripts/ci/check_suites_h1h2.sh`

## Phase 4 curriculum generation

- Generate: `python3 scripts/bench/generate_phase4_curriculum.py`
- Check up to date: `python3 scripts/bench/generate_phase4_curriculum.py --check`

## Fixtures

Filesystem fixture snapshots live under:

`benchmarks/fixtures/fs/solve-fs/<suite_id>/root/`

Build a stable, read-only snapshot (normalized mtimes + deterministic manifest):

`./scripts/build_fixture_fs_tree.sh <fixture_id> <source_dir>`

Request/response fixtures live under:

- `benchmarks/fixtures/rr/solve-rr/<suite_id>/index.json` + `benchmarks/fixtures/rr/solve-rr/<suite_id>/bodies/**`.

Key/value fixtures live under:

- `benchmarks/fixtures/kv/solve-kv/<suite_id>/seed.json`

Solve-pure fixture blobs (for large inputs/outputs) live under:

- `benchmarks/fixtures/pure/<world>/<suite_id>/`

## Properties

Notes on holdouts and anti-overfit checks: `benchmarks/properties/README.md`
