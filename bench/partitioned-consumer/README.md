# Service benchmark: partitioned-consumer lag lab

This harness is a reproducible lab for consumer lag and catch-up behavior.

## Output schema

The harness writes a single JSON file with:

- `schema_version`: `x07.service_bench.partitioned_consumer@0.1.0`
- `kind`: `partitioned-consumer`
- `lag_samples`: array of `{ unix_s, lag }`
- `catch_up_s`: integer
- `throughput_eps`: number

## Canonical usage (dry mode)

```sh
BENCH_DRY=1 ./bench/partitioned-consumer/run.sh --out /tmp/partitioned-consumer.json
```

## Expert usage (real lab)

Real mode typically requires a broker (Kafka-compatible) and a controlled producer/consumer deployment.
Keep the dry mode stable for CI.
