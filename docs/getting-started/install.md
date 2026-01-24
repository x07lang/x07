# Install X07

X07 ships as a small toolchain with stable, JSON-first contracts:

- `x07` — canonical CLI (init, ast, fmt/lint/fix, run, test, pkg, build)
- `x07c` — compiler backend (advanced)
- `x07-host-runner` — deterministic runner backend (solve-* worlds; advanced)
- `x07-os-runner` — OS runner backend (run-os* worlds; advanced)

The recommended installer is `x07up` (toolchain manager). It installs:

- the toolchain under `~/.x07/toolchains/<tag>/`
- shims under `~/.x07/bin/` (put this on `PATH`)
- offline docs + the Codex skills pack (default profile)

Quickstart and full installer reference:

- quickstart: [Installer / x07up](installer.md)

## Supported platforms

X07 aims to be usable on:

- macOS (Apple Silicon + Intel)
- Linux (glibc + musl variants)
- Windows

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

### Windows

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "iwr -useb https://x07lang.org/install.ps1 -OutFile install.ps1; .\\install.ps1 -Yes -Channel stable"
```

CI / agent-friendly:

```powershell
.\install.ps1 -Yes -Channel stable -NoModifyPath -Json
```

### Verify

Verify:

- `x07 --version`
- `x07up --version`
- `x07 --help`
- `x07 run --help`
- `x07 ast apply-patch --help`
- `x07up doctor --json` (toolchain + host prerequisites)
- optional (advanced): `x07c --help`, `x07-host-runner --help`, `x07-os-runner --help`

### Pin a toolchain per project

Write `x07-toolchain.toml` in your repo root:

```bash
x07up override set v0.0.30
```

This makes toolchain selection deterministic for agents and CI.

## Option B (advanced): manual install

If you cannot use `x07up`, download and unpack a toolchain archive and put its `bin/` directory on `PATH`.

## Option C (advanced): build from source (Rust toolchain)

If you have `cargo` installed:

- `cargo build -p x07 -p x07c -p x07-host-runner -p x07-os-runner --release`
- the resulting binaries will be in `target/release/`

Then verify with the same commands above.

## Troubleshooting

### “Permission denied” on macOS

If Gatekeeper blocks the binary:

- remove the quarantine attribute on the unpacked binaries (or sign them in your org).

### “C compiler not found”

X07 compiles to C and invokes a system C compiler.

Run `x07up doctor --json` for a machine-readable environment report and install suggestions.

Install:

- macOS: Xcode Command Line Tools
- Linux: `clang` or `gcc`
- Windows: MSVC Build Tools or clang-cl

### “I can run `x07` but `x07 run` fails”

`x07 run` dispatches to `x07-host-runner` / `x07-os-runner`.

- ensure the runner binaries are on your `PATH` (or installed next to `x07`)
- ensure a working C compiler is available (override via `X07_CC`)

### “could not locate stdlib/os module root”

`x07-os-runner` expects the toolchain install tree to include `stdlib/os/0.2.0/modules`.

- If you downloaded a release archive, ensure `stdlib/` exists next to `bin/`.
- If you built from source, run `x07-os-runner` from the `x07/` repo root (so it can find `stdlib/os/...`).
