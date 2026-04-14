# Profiling (minimal, deterministic)

X07 includes a minimal function profiler intended for reproducible “what got slower?” investigations without relying on wall-clock timing.

Enable it by setting:

- `X07_PROFILE=1`

Note: this profiler is compile-time instrumentation. `X07_PROFILE` must be set when the program is compiled/bundled. For `x07 run`, setting it on the same command works because `x07 run` compiles then executes.

When enabled, the generated runtime prints one JSON line to stderr at program exit:

- `schema_version: "x07.profile.fn@0.1.0"`
- per-function counters:
  - `calls`
  - `fuel_excl` (exclusive fuel; deterministic “time” unit)
  - `alloc_calls_excl`
  - `alloc_bytes_excl`

## Usage

Example:

```bash
X07_PROFILE=1 x07 run --profile os
```

## Notes

- The unit is **fuel**, not wall time. Fuel is deterministic for deterministic inputs and code paths.
- The profiler instruments `solve` and user-defined `defn` functions.
- Native backends (for example filesystem/archive adapters) are not included in these counters.
- If the process terminates early (for example a trap), the profile line may not be emitted.
