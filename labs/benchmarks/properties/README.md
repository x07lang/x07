# Benchmark Properties (Anti-Overfit)

Benchmark suites support additional assertions beyond “bytes in → bytes out” to discourage accidental regressions and non-deterministic behavior.

The regression runner (`labs/scripts/bench/run_bench_suite.py`) enforces only assertions that map to deterministic runner stats, including:

- Call count ranges (for example: `min_fs_read_file_calls`, `exact_rr_requests`, `min_kv_get_calls`).
- Deterministic replay checks (`replay_required`, `replay_runs`).
- Memory stats gates (`mem_stats_required`, `leak_free_required`, and max-* limits).
