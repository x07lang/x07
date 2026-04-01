# MCP quality: install

`x07-mcp-test` is a standalone verifier CLI. The private alpha is distributed as prebuilt binaries from GitHub Releases.

Conformance requires a working Node.js toolchain (the official suite runs via `npx`). Use `x07-mcp-test doctor` to check your environment before you run conformance in CI.

## Install from release artifacts

1) Download the release asset for your OS/arch from `x07lang/x07-mcp-test` GitHub Releases.

2) Extract the archive and place `x07-mcp-test` on your `PATH`.

3) Verify installation:

```sh
x07-mcp-test --help
x07-mcp-test doctor
```

## Codespaces install helper

If you are evaluating via `x07lang/x07-mcp` Codespaces, the repo includes a helper that downloads a pinned alpha verifier binary:

```sh
./scripts/dev/install_x07_mcp_test.sh
```

Then run:

```sh
~/.local/bin/x07-mcp-test --help
```

