# Installer / x07up

X07’s canonical installer is `x07up` (toolchain manager).

The bootstrap scripts (`install.sh` / `install.ps1`) install `x07up`, then `x07up` installs the selected toolchain and sets up `~/.x07/bin/` shims.

Quickstart: see [Install](install.md).

## One-command install

macOS / Linux:

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- --yes --channel stable
```

Then (in the same shell session):

```bash
export PATH="$HOME/.x07/bin:$PATH"
```

Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "iwr -useb https://x07lang.org/install.ps1 -OutFile install.ps1; .\\install.ps1 -Yes -Channel stable"
```

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
x07up override set v0.0.28
```

This writes a file like:

```toml
[toolchain]
channel = "v0.0.28"
components = ["docs", "skills"]
```

Notes:

- `channel` can be `stable` or a specific tag like `v0.0.28`.
- `components` controls whether `x07up` installs the offline docs and skills pack.

Remove it:

```bash
x07up override unset
```

## Agent kit (offline docs + skills + AGENT.md)

- Offline docs path: `x07up docs path --json`
- Skills install:
  - user-scoped: `x07up skills install --user`
  - project-scoped: `x07up skills install --project .`
- Project rails: `x07up agent init --project . --pin stable --with-skills project`

See also: [Available skills](available-skills.md).

## If you’re stuck

Run:

```bash
x07up doctor --json
```

It reports missing prerequisites and broken toolchain layout in a machine-readable way.
