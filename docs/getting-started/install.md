# Install X07

X07 ships as a small toolchain with stable, JSON-first contracts:

- `x07` — canonical CLI (init, ast, fmt/lint/fix, run, test, pkg, build)
- `x07c` — compiler backend (advanced)
- `x07-os-runner` — OS execution backend (advanced)

The recommended installer is `x07up` (toolchain manager). It installs:

- the toolchain under `~/.x07/toolchains/<tag>/`
- shims under `~/.x07/bin/` (put this on `PATH`)
- offline docs + the agent skills pack (default profile)

Quickstart and full installer reference:

- quickstart: [Installer / x07up](installer.md)

## Prerequisites

- macOS / Linux bootstrap installer (`install.sh`): `sh`, `tar`, and either `curl` or `wget` (plus a sha256 tool: `sha256sum`, `shasum`, or `openssl`)
- A C compiler toolchain (`clang`/`gcc` on macOS/Linux)
- Linux: libcurl development headers for packages that link libcurl (for example `ext-curl-c`)

After installing, run `x07 doctor` for a machine-readable environment report and install suggestions.

## Supported platforms

Prebuilt toolchains (via `x07up`) are currently published for:

- macOS (Apple Silicon + Intel)
- Linux (x86_64 glibc + ARM64 glibc)

On Windows, X07 is supported via WSL2 (run X07 inside a Linux distro such as Ubuntu and follow the Linux instructions below). Native Windows toolchains are not currently published.

Other targets (for example Linux musl/Alpine) currently require building from source.

See [platform support and smoke tests](../worlds/os-worlds.md#platform-support).

## Option A (recommended): install via `x07up`

### macOS / Linux

Install `x07up` + the stable toolchain:

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- --yes --channel stable
```

Then (in the same shell session), ensure `x07` is on `PATH`:

```bash
export PATH="$HOME/.x07/bin:$PATH"
```

If you omit `--no-modify-path`, the installer also attempts to persist this for future shells (for zsh on macOS: `~/.zprofile`).

CI / agent-friendly (no profile edits, JSON report):

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- \
  --yes \
  --channel stable \
  --no-modify-path \
  --json
```

### Windows (WSL2)

X07 does not currently publish native Windows toolchains. On Windows, install and run X07 inside WSL2:

1. Install WSL2 (Ubuntu recommended).
2. In your WSL2 shell, follow the macOS / Linux install instructions above.

### Verify

Verify:

- `x07 --version`
- `x07up --version`
- `x07 --help`
- `x07 run --help`
- `x07 ast apply-patch --help`
- `x07 doctor` (host prerequisites for OS worlds)
- `x07up doctor --json` (installer/toolchain + host prerequisites)
- optional (advanced): `x07c --help`, `x07-os-runner --help`

### Pin a toolchain per project

Write `x07-toolchain.toml` in your repo root:

```bash
x07up override set v0.1.26
```

This makes toolchain selection deterministic for agents and CI.

## Option B (advanced): manual install

If you cannot use `x07up`, download and unpack a toolchain archive and put its `bin/` directory on `PATH`.

## Option C (advanced): build from source (Rust toolchain)

If you have `cargo` installed:

- `cargo build -p x07 -p x07c -p x07-os-runner --release`
- the resulting binaries will be in `target/release/`

Then verify with the same commands above.

## Troubleshooting

### “Permission denied” on macOS

If Gatekeeper blocks the binary:

- remove the quarantine attribute on the unpacked binaries (or sign them in your org).

### “C compiler not found”

X07 compiles to C and invokes a system C compiler.

Run `x07 doctor` for a machine-readable environment report and install suggestions.

Install:

- macOS: Xcode Command Line Tools
- Linux: `clang` or `gcc`
- Windows (WSL2): install a Linux toolchain in your distro (for example Ubuntu: `sudo apt-get install -y clang build-essential`)

### “clang: error: no such file or directory: 'SystemConfiguration'” (macOS)

This can happen if the toolchain emits malformed macOS framework link flags.

Upgrade to the latest stable toolchain (for example: `x07up update`), then re-run. If it still fails on the latest toolchain, file a bug with the full `stderr_b64` decoded output from the run report.

### “I can run `x07` but `x07 run` fails”

`x07 run` invokes runner backends that ship with the toolchain.

- ensure the runner binaries are on your `PATH` (or installed next to `x07`)
- ensure a working C compiler is available (override via `X07_CC`)

### “could not locate stdlib/os module root”

`x07-os-runner` expects the toolchain install tree to include `stdlib/os/0.2.0/modules`.

- If you downloaded a release archive, ensure `stdlib/` exists next to `bin/`.
- If you built from source, run `x07-os-runner` from the `x07/` repo root (so it can find `stdlib/os/...`).
