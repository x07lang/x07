# Repositories

The `x07lang` GitHub org is split into a few small repos so that end-user tooling, infrastructure, and documentation stay focused.

- `x07lang/x07`: the canonical toolchain repo (compiler, runners, stdlib/packages source, schemas/specs, docs).
- `x07lang/x07-platform-contracts`: authoritative public `lp.*` platform contracts, schema indexes, and registry mirror helpers.
- `x07lang/x07-platform`: public lifecycle runtime, CLI, self-hosted control plane, adapters, fixtures, and platform docs.
- `x07lang/x07-platform-cloud`: private hosted-product layer that consumes the public platform repos without forking them.
- `x07lang/x07-mcp`: MCP kit tooling, templates, reference servers, and the official `io.x07/x07lang-mcp` bridge for structured X07 lifecycle and ecosystem MCP actions.
- `x07lang/x07-website`: the x07lang.org site built from released docs bundles.
- `x07lang/x07-registry`: package registry server implementation.
- `x07lang/x07-registry-web`: the package registry UI (x07.io).
- `x07lang/x07-perf-compare`: optional performance comparison harnesses and experiments.

The `x07lang/x07-infra-private` repo contains private infrastructure and runbooks.
