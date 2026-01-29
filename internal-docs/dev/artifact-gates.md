# Artifact gates

Artifact gates are fast, deterministic checks that run before executing an untrusted native solver artifact.

## What is gated

Pre-run checks (runner):

- The artifact exists, is a regular file, is executable, and is below a size cap.
- The artifact exists, is a regular file, and is executable.

Runtime validation (runner):

- Stdout must obey the solver ABI (length-prefixed bytes).
- Metrics are parsed from stderr (last JSON line with `fuel_used`).
- Stdout/stderr are capped to prevent output-spam from exhausting host memory.

## Where it runs

- Pre-run gate + runtime validation: `crates/x07-host-runner` (`run_artifact_file`, `parse_native_stdout`, `parse_metrics_fuel_used`).
- Toolchain availability: `./scripts/ci/check_tools.sh`.
