# Performance Comparison: X07 vs C vs Rust

This directory contains benchmark programs to compare performance and memory usage
between X07, C, and Rust implementations of identical algorithms.

## Benchmarks

| Benchmark | Description |
|-----------|-------------|
| `sum_bytes` | Sum all bytes in input, output as u32_le |
| `word_count` | Count whitespace-separated words |
| `rle_encode` | Run-length encoding of byte stream |
| `byte_freq` | Byte frequency histogram (byte -> count mapping) |
| `fibonacci` | Iterative Fibonacci computation |
| `regex_is_match` | Regex `is_match` over generated ASCII text |
| `regex_count` | Regex `find_all` and count matches |
| `regex_replace` | Regex `replace_all` with a short replacement |

## Running Benchmarks

```bash
# Run all benchmarks with default settings (100KB input, 5 iterations)
python3 perf-compare/run_benchmarks.py

# Run with custom input size (in KB)
python3 perf-compare/run_benchmarks.py --size 1000

# Run specific benchmarks
python3 perf-compare/run_benchmarks.py --benchmarks sum_bytes word_count

# More iterations for better statistics
python3 perf-compare/run_benchmarks.py --iterations 10 --warmup 3

# Output as JSON
python3 perf-compare/run_benchmarks.py --json > results.json

# Direct binary execution (no host runner overhead)
python3 perf-compare/run_benchmarks.py --direct

# Size-focused native builds (passes through to x07-host-runner --cc-profile)
python3 perf-compare/run_benchmarks.py --x07-cc-profile size

# Regex benchmarks require the native ext-regex backend staged into `deps/`
./scripts/build_ext_regex.sh

# Run against a different checkout of the repo (useful for before/after comparisons)
python3 perf-compare/run_benchmarks.py --repo-root /path/to/x07
```

## Options

- `--size N`: Input size in KB (default: 100)
- `--iterations N`: Number of timed runs (default: 5)
- `--warmup N`: Warmup iterations before timing (default: 2)
- `--benchmarks`: Specific benchmarks to run
- `--json`: Output results as JSON
- `--direct`: Run X07 binaries directly without host runner overhead
- `--x07-cc-profile {default,size}`: Select the C toolchain profile for X07 builds
- `--repo-root`: Override repo root (for comparing two checkouts)

## Directory Structure

```
perf-compare/
├── README.md
├── run_benchmarks.py    # Benchmark runner script
├── x07/             # X07 programs (.x07.json)
├── projects/            # Project-based X07 benchmarks (packages)
├── c/                   # C implementations
├── rust/                # Rust implementations (single-file)
└── rust_cargo/           # Rust implementations with Cargo deps
```

## Requirements

- Python 3.10+
- Rust toolchain (rustc)
- C compiler (cc/gcc/clang)
- Built X07 toolchain (`cargo build --release`)

## Metrics Collected

- **Mean time**: Average execution time across iterations
- **Min time**: Minimum (best) execution time
- **StdDev**: Standard deviation of execution times
- **Compile time**: One-time compilation overhead
- **Build size**: Final executable size (bytes on disk)
- **Peak RSS**: Peak resident set size from a single run (via `/usr/bin/time` if available)
- **Speedup**: Relative performance vs X07 (higher = faster)

## Notes

- **Host runner mode** (default): X07 times include host runner overhead (process spawn, JSON I/O, artifact loading). This represents the full execution environment overhead.
- **Direct binary mode** (`--direct`): Runs compiled X07 binaries directly, measuring only native code execution time. This gives a fairer comparison of generated code quality.
- **Peak RSS**: Measured by executing the compiled binaries directly once per language (so the metric is comparable across languages, independent of X07 host runner overhead).
- C and Rust programs are compiled with `-O3` / `opt-level=3`
- All programs use stdin/stdout for I/O
- Output correctness is verified (outputs must match across languages)

## Performance Comparison (January 2026)

After recent optimizations, X07 is competitive with C and Rust, with the **fastest compile times**.

### Direct Binary Mode (1MB input)

| Benchmark | X07 | C | Rust | C vs X07 | Rust vs X07 |
|-----------|---------|---|------|--------------|-----------------|
| sum_bytes | 7.94ms | 13.49ms | 5.86ms | **0.59x** (X07 faster) | 1.35x |
| word_count | 9.26ms | 14.05ms | 6.61ms | **0.66x** (X07 faster) | 1.40x |
| rle_encode | 7.62ms | 16.00ms | 6.79ms | **0.48x** (X07 faster) | 1.12x |
| byte_freq | 14.89ms | 13.47ms | 6.15ms | 1.11x | 2.42x |
| fibonacci | 2.21ms | 1.63ms | 2.05ms | 1.36x | 1.08x |

### Compile Times

| Benchmark | X07 | C | Rust |
|-----------|---------|---|------|
| sum_bytes | **20.2ms** | 47.3ms | 91.2ms |
| word_count | **13.8ms** | 43.3ms | 92.3ms |
| rle_encode | **14.7ms** | 43.7ms | 97.8ms |
| byte_freq | **14.9ms** | 48.9ms | 101.8ms |
| fibonacci | **15.4ms** | 41.6ms | 91.9ms |

X07 compiles **2-3x faster than C** and **6-7x faster than Rust**.

### Build Size & Memory

| Metric | X07 | C | Rust |
|--------|---------|---|------|
| Binary Size | ~33.6 KiB | ~32.8 KiB | ~448 KiB |
| Peak RSS | 1.2-4.2 MiB | 1.2-2.4 MiB | 1.4-2.6 MiB |

### Key Findings

1. **Fastest compile times**: X07 compiles in 14-20ms, 2-3x faster than C and 6-7x faster than Rust.

2. **X07 beats C in I/O-heavy benchmarks**: For byte-stream processing (sum_bytes, word_count, rle_encode), X07 is **1.5-2.1x faster than C**.

3. **Rust remains fastest for vectorizable workloads**: Rust's SIMD optimization provides 1.1-2.4x speedup over X07 in most benchmarks.

4. **Compact binaries**: X07 produces ~33.6 KiB binaries, comparable to C (~32.8 KiB) and 13x smaller than Rust (~448 KiB).

5. **Memory efficient**: Peak RSS is comparable across all three languages (1-4 MiB range).

### Summary

| Metric | X07 vs C | X07 vs Rust |
|--------|--------------|-----------------|
| Compile Time | **X07 2-3x faster** | **X07 6-7x faster** |
| I/O-Heavy Runtime | **X07 1.5-2.1x faster** | Rust 1.1-1.4x faster |
| Compute Runtime | C 1.4x faster | Rust 1.1x faster |
| Binary Size | Comparable | **X07 13x smaller** |

X07 offers the best compile times while producing compact, high-performance binaries that outperform C in I/O-bound workloads.
