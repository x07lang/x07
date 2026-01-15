# Cross-platform tests (CI)

This repo compiles x07AST → C and executes native binaries, so “works on my machine” is not enough. Cross-platform CI is meant to:

- catch toolchain + runner portability regressions (Linux/macOS/Windows)
- catch native-linking regressions in OS-world external packages (TLS/curl/zlib/etc.)
- keep one canonical command for agents and humans

## Status (implemented in-tree)

- Canonical gate: `./scripts/ci/check_all.sh` (bash) and `./scripts/ci/check_all.ps1` (Windows wrapper).
- GitHub Actions:
  - Tier 1 (PR gate): `.github/workflows/ci-unix.yml`, `.github/workflows/ci-windows.yml` (Windows native)
  - Tier 2 (nightly/manual distro coverage): `.github/workflows/ci-linux-containers.yml`

## One canonical gate

Run this everywhere (local + CI):

- `./scripts/ci/check_all.sh`

On Windows (PowerShell), you can also run:

- `./scripts/ci/check_all.ps1` (requires `bash` in `PATH`, e.g. MSYS2 UCRT64 or WSL2)

### What `check_all` runs

In order:

1. `./scripts/ci/check_tools.sh`
2. `cargo fmt --check`
3. `cargo test`
4. `cargo clippy --all-targets -- -D warnings`
5. `./scripts/ci/check_external_packages_lock.sh`
6. `./scripts/ci/check_canaries.sh`
7. `./scripts/ci/check_external_packages_os_smoke.sh`

Output contract:

- prints `==> <step>` before each step
- prints `ok: ...` lines from sub-gates
- exits non-zero on the first failing step
- when an OS-world smoke fails, prints compile/run diagnostics (trap + stderr) via `scripts/ci/assert_run_os_ok.py` (and for the HTTP server smokes, also prints the background server stderr captured in `tmp/tmp/http_server_*.stderr`)

OS-world note:

- `scripts/ci/check_external_packages_os_smoke.sh` uses a repo fixture file (`tests/external_os/fixtures/file_url_source.txt`) for `file://` HTTP tests to avoid platform-specific paths like `/etc/hosts`.
- The sandboxed `file://` policy (`tests/external_os/net/run-os-policy.file-etc-allow-ffi.json`) allows `PWD`/`GITHUB_WORKSPACE` so tests can build a deterministic absolute `file:///...` URL to that fixture.
- The loopback HTTP server smokes run a background `run-os` process and use a longer client connect timeout to tolerate slower cold-start native compilation (especially on Windows).

## Debugging CI failures (gh)

Use `gh` (authenticated) to inspect failures without opening the web UI:

- List recent Windows runs: `GH_PAGER=cat gh run list --workflow "CI (Windows)" --limit 10`
- View only failed step logs: `GH_PAGER=cat gh run view <run_id> --log-failed`
- View the full log: `GH_PAGER=cat gh run view <run_id> --log`

Line endings note:

- The repo enforces LF via `.gitattributes` so `x07import` `source_sha256` checks are stable on Windows; if you see `x07import source sha256 mismatch`, re-checkout with attributes applied.

Windows stack note:

- On Windows, Rust binaries default to a ~1MiB stack. If you see `thread 'main' has overflowed its stack` (often with exit code `3221225725` / `0xC00000FD`) in canary benches, it’s usually a stack-depth issue in the compile→run path.
- You can reproduce “Windows-like” stack limits on Unix to debug locally: `ulimit -s 1024; python3 scripts/bench/run_bench_suite.py --suite benchmarks/solve-full/phaseH2-smoke.json --solutions benchmarks/solutions`.

## CI matrix

### Tier 1 (release/PR blocking)

Runs `./scripts/ci/check_all.sh` on real OS runners:

- Ubuntu: `ubuntu-22.04`, `ubuntu-24.04`
- macOS: `macos-14`
- Windows: `windows-latest` (MSYS2 UCRT64 toolchain)

WSL2 note:

- A WSL2 job exists in `.github/workflows/ci-windows.yml` and runs nightly (plus `workflow_dispatch`), but it is not PR-blocking. GitHub-hosted Windows runners can fail to start WSL2 VMs (`WSL_E_VM_MODE_INVALID_STATE`); for reliable WSL2 CI coverage you likely need a self-hosted runner with WSL2 enabled.

### Tier 2 (distro drift)

Runs the same gate inside a Linux container (nightly + manual trigger):

- `debian:bookworm`

## Local prerequisites (to match CI)

`check_all` includes OS-world external package smokes, so you need the system libraries they link against.

### Ubuntu / Debian

Install:

- `clang`
- `build-essential`
- `pkg-config`
- `libssl-dev`
- `libcurl4-openssl-dev`
- `zlib1g-dev`
- `libsqlite3-dev`

Example:

- `sudo apt-get update`
- `sudo apt-get install -y clang build-essential pkg-config libssl-dev libcurl4-openssl-dev zlib1g-dev libsqlite3-dev`

### macOS

Install:

- `brew install openssl@3 pkg-config`

### Windows

Use one of:

- MSYS2 UCRT64 (matches `.github/workflows/ci-windows.yml`)
- WSL2 (run as Linux)

## How to extend cross-platform coverage (without duplicating CI logic)

- Add new OS-world external package smokes to `scripts/ci/check_external_packages_os_smoke.sh`.
- Keep `scripts/ci/check_all.sh` as the only “entrypoint” command; CI should call it rather than re-encoding logic in workflow YAML.
- If a new smoke needs a new system library, update the per-OS install steps in the workflows (and this doc).
