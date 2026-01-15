# Benchmark reference solutions

This directory contains **committed reference programs** for benchmark `task_id`s.

They are used by `scripts/bench/run_bench_suite.py` to run deterministic regression
checks without any LLM-driven synthesis loop.

Layout:

- One file per `task_id`, stored as `benchmarks/solutions/<task_id>.x07.json`
  (for example: `benchmarks/solutions/smoke/echo.x07.json`).
