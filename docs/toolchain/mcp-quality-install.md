# MCP quality: install

Hardproof (`hardproof`) is a standalone verifier CLI. The public beta is distributed as prebuilt binaries from GitHub Releases.

Conformance requires a working Node.js toolchain (the official suite runs via `npx`). Use `hardproof doctor` to check your environment before you run conformance in CI.

## Install from release artifacts

1) Download the release asset for your OS/arch from `x07lang/hardproof` GitHub Releases.

2) Extract the archive and place `hardproof` on your `PATH`.

3) Verify installation:

```sh
hardproof --help
hardproof doctor
```

## Codespaces install helper

If you are evaluating via `x07lang/x07-mcp` Codespaces, the repo includes a helper that downloads a pinned alpha verifier binary:

```sh
./scripts/dev/install_hardproof.sh
```

Then run:

```sh
~/.local/bin/hardproof --help
```
