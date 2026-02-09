# perf-compare

The performance comparison harnesses were moved out of this repo to keep the core toolchain checkout lean:

- `x07lang/x07-perf-compare`: https://github.com/x07lang/x07-perf-compare

That repo tracks cross-language comparisons (X07 vs C vs Rust vs Go).

## Running

Clone `x07-perf-compare` next to an `x07` checkout (or set `X07_REPO_ROOT`) and run:

```bash
python3 run_benchmarks.py
```

See the `x07-perf-compare` repo README for benchmark options and requirements.
