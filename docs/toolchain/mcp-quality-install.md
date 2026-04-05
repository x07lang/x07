# MCP quality: install

Hardproof (`hardproof`) is a standalone verifier CLI. The public beta is distributed as prebuilt binaries from GitHub Releases.

Scan runs in the `hardproof` binary (no Node.js toolchain required). Use `hardproof doctor` to check your environment before you run verification in CI.

## Install from release artifacts

1) Download the release asset for your OS/arch from `x07lang/hardproof` GitHub Releases.

2) Extract the archive and place `hardproof` on your `PATH`.

3) Verify installation:

```sh
hardproof --help
hardproof doctor
```

## Codespaces install helper

If you are evaluating via `x07lang/x07-mcp` Codespaces, the repo includes a helper that installs the verifier binary (defaults to the latest beta, or you can pass a specific tag):

```sh
./scripts/dev/install_hardproof.sh
```

Then run:

```sh
~/.local/bin/hardproof --help
```
