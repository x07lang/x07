# Service benchmark: replicated-http

This harness measures request latency/throughput for a replicated HTTP cell.

## Output schema

The harness writes a single JSON file with:

- `schema_version`: `x07.service_bench.replicated_http@0.1.0`
- `kind`: `replicated-http`
- `window_s`: integer
- `rps`: number
- `latency_ms`: object with `p50`, `p95`, `p99`
- `error_rate`: number in `[0,1]`

## Canonical usage (dry mode)

CI runs this harness in dry mode (no cluster required):

```sh
BENCH_DRY=1 ./bench/replicated-http/run.sh --out /tmp/replicated-http.json
```

## Expert usage (real load)

This repo does not vendor a load generator. Recommended:

- `vegeta` for HTTP load generation
- a Kubernetes job or a controlled local runner for repeatability

Wire the real runner behind `./bench/replicated-http/run.sh` and keep `BENCH_DRY=1` stable for CI.
