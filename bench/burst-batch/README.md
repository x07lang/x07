# Service benchmark: burst-batch recovery

This harness validates and benchmarks crash/restart recovery for batch/job workloads.

## Output schema

The harness writes a single JSON file with:

- `schema_version`: `x07.service_bench.burst_batch@0.1.0`
- `kind`: `burst-batch`
- `runs`: integer
- `crashes_injected`: integer
- `duplicate_completions`: integer (must be `0`)

## Canonical usage (dry mode)

```sh
BENCH_DRY=1 ./bench/burst-batch/run.sh --out /tmp/burst-batch.json
```

## Expert usage (real recovery test)

Real mode requires a workload and a crash injector. Keep the dry mode stable for CI.
