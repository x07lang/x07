# Benchmarks (`x07 bench`)

`x07 bench` is the patch-centric benchmark harness for agent correctness loops.

It evaluates whether a candidate patch actually resolves an instance, with deterministic artifacts and machine-readable reports.

Related benchmark surfaces:

- Performance regression canaries are tracked in CI (internal).
- `x07-perf-compare`: cross-language performance comparisons (X07 vs C vs Rust vs Go).

## Commands

- `x07 bench list --suite <suite.json>`
- `x07 bench validate --suite <suite.json>`
- `x07 bench eval --suite <suite.json> --predictions <predictions.jsonl>`
- `x07 bench eval --suite <suite.json> --oracle`
- `x07 bench eval --suite <suite.json> --predictions <predictions.jsonl> --runner docker`

Seed suites shipped in the toolchain repo:

- `labs/x07bench/suites/core_v0/` (minimal)
- `labs/x07bench/suites/core_v1/` (expanded; recommended)

## Bench suite layout

A suite is a directory tree rooted at `suite.json`:

```text
labs/x07bench/
  suites/
    core_v1/
      suite.json
      instances/
        std_math_0001/
          instance.json
          issue.md
          repo/
          oracle.patchset.json
```

## Schemas

- `spec/x07-bench.suite.schema.json` (`x07.bench.suite@0.1.0`)
- `spec/x07-bench.instance.schema.json` (`x07.bench.instance@0.1.0`)
- `spec/x07-bench.report.schema.json` (`x07.bench.report@0.1.0`)

Predictions JSONL supports:

- `patch_kind: "x07-arch-patchset-json"` (primary)
- `patch_kind: "unified-diff"` (compat)

## Evaluation protocol

Per instance, `x07 bench` runs:

1. Baseline `x07 test` (must fail)
2. Apply patch
3. Optional repair on touched `*.x07.json`
4. Post-patch `x07 test`
5. Optional determinism rerun checks

## Output

`x07 bench eval` emits `x07.bench.report@0.1.0`.

Primary KPIs:

- `resolved / instances_total`
- `resolved_without_errors`
- repair iteration/ops averages

Additional metrics (derived from `x07.bench.report@0.1.0`):

- **first-pass compile success rate:** fraction of instances where `after_patch.compile_failures == 0`
- **mean lint→fix iterations:** mean of `repair.iterations` (per touched `*.x07.json`)
- **repair ops:** mean of `repair.applied_ops_count`

Helper script (toolchain repo):

```sh
python3 labs/x07bench/scripts/score_report.py --in <bench.report.json>
```

## Docker path

Use `--runner docker` for in-command containerized evaluation:

```sh
x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json --oracle --runner docker
```

`x07 bench` delegates to `ci/x07bench/run.sh` under the hood. You can also call the wrapper directly:

```sh
ci/x07bench/run.sh bench eval --suite labs/x07bench/suites/core_v1/suite.json --oracle
```
