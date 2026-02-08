# Installer / x07up

X07’s canonical installer is `x07up` (toolchain manager).

The bootstrap script (`install.sh`) installs `x07up`, then `x07up` installs the selected toolchain and sets up `~/.x07/bin/` shims. On Windows, run the bootstrap inside WSL2.

Quickstart: see [Install](install.md).

## Prerequisites

- macOS / Linux: `sh`, `tar`, and either `curl` or `wget` (plus a sha256 tool: `sha256sum`, `shasum`, or `openssl`)
- For compiling/running X07 programs: a C toolchain (`x07up doctor --json` reports missing deps)
- Linux: libcurl development headers for packages that link libcurl (for example `ext-curl-c`)

## One-command install

macOS / Linux:

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- --yes --channel stable
```

Then (in the same shell session):

```bash
export PATH="$HOME/.x07/bin:$PATH"
```

Windows (WSL2):

X07 does not currently publish native Windows toolchains. Install and run X07 inside WSL2 (Ubuntu recommended) and use the macOS / Linux installer command above.

## CI / agent mode (deterministic + machine-clean)

In CI, avoid modifying shell profiles and emit a single JSON report:

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- \
  --yes \
  --channel stable \
  --no-modify-path \
  --json
```

## Toolchain selection (deterministic precedence)

`x07up` selects the active toolchain in this order:

1. `X07UP_TOOLCHAIN` environment variable
2. `x07-toolchain.toml` found by walking up directories
3. global default from `~/.x07/config.json`
4. fallback: `stable` (but `x07up` never auto-installs; you must run `x07up install`)

## Pinning per project

Write `x07-toolchain.toml`:

```bash
x07up override set v0.1.8
```

This writes a file like:

```toml
[toolchain]
channel = "v0.1.8"
components = ["docs", "skills"]
```

Notes:

- `channel` can be `stable` or a specific tag like `v0.1.8`.
- `components` controls whether `x07up` installs the offline docs and skills pack.

Remove it:

```bash
x07up override unset
```

## Agent kit (offline docs + skills + project rails)

- Offline docs path: `x07up docs path --json`
- Skills install:
  - user-scoped: `x07up skills install --user`
  - project-scoped: `x07up skills install --project .`
- Canonical project start (creates `AGENT.md`, pins the toolchain, and installs project-scoped skills): `x07 init`

See also: [Available skills](available-skills.md).

## If you’re stuck

Run:

```bash
x07up doctor --json
```

It reports missing prerequisites and broken toolchain layout in a machine-readable way.
