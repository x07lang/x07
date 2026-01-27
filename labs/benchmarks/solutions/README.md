# Benchmark reference solutions

This directory contains **committed reference programs** for benchmark `task_id`s.

They are used by `labs/scripts/bench/run_bench_suite.py` to run deterministic regression
checks without any LLM-driven synthesis loop.

Layout:

- One file per `task_id`, stored as `labs/benchmarks/solutions/<task_id>.x07.json`
  (for example: `labs/benchmarks/solutions/smoke/echo.x07.json`).
