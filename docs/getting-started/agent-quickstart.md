# Agent quickstart (learn X07 from scratch)

This page is a single entry point for LLM agents. X07 is a new language and most LLMs have little to no prior training data on it, so treat the repository + docs as the ground truth.

## 0) The mental model

- X07 source is **x07AST JSON** (`*.x07.json`), not text.
- The toolchain is **JSON-first**: diagnostics, patches, and reports are structured.
- Deterministic development happens in `solve-*` worlds; real OS access is `run-os*`.

## 1) Install and verify the toolchain

- Download the latest release archive from https://github.com/x07lang/x07/releases/latest
- Add its `bin/` directory to `PATH`.

Verify:

```bash
x07 --help
x07 run --help
x07 --cli-specrows
```

If you plan to use OS worlds (`run-os*`) with native deps (for example `ext-net` / `ext-curl-c` / `ext-sockets-c`), run:

```bash
x07 doctor
```

See also: [Install](install.md).

## 2) Create a project (canonical starting point)

```bash
mkdir myapp
cd myapp
x07 --init
```

This creates:

- `x07.json` (with `test`, `os`, and `sandbox` profiles)
- `x07.lock.json`
- `src/` (a minimal program)
- `tests/tests.json` + `tests/smoke.x07.json` (a deterministic harness smoke test)

If you are creating a publishable package, use `x07 --init --package` to also create `x07-package.json`.

## 3) The core loop: fmt → lint → run → test

Format and lint a single file:

```bash
x07 fmt --input src/main.x07.json --write
x07 lint --input src/main.x07.json --world solve-pure
```

Run:

```bash
x07 run
```

If your program expects CLI arguments via `argv_v1`, pass them after `--` and `x07 run` will encode them into input bytes:

```bash
x07 run --profile os -- tool --help
```

Run the deterministic harness (repo-defined suites):

```bash
x07 test --manifest tests/tests.json
```

## 4) Repairs: quickfixes and patches

Two canonical repair mechanisms:

- **Quickfixes**: `x07 fix` applies the tool-provided `quickfix` JSON patches deterministically.
- **Explicit patches**: `x07 ast apply-patch` applies RFC 6902 JSON Patch.

Example loop:

```bash
x07 lint --input src/main.x07.json --world solve-pure
x07 fix --input src/main.x07.json --world solve-pure --write
x07 lint --input src/main.x07.json --world solve-pure
```

## 5) Learn the language

- Start here: [Language overview](../language/overview.md)
- Read the concrete syntax model: [Syntax & x07AST](../language/syntax-x07ast.md)

Key x07AST shapes:

- expression call: `["head", arg1, arg2, ...]`
- identifier: `"std.bytes.len"`

## 6) Packages (external dependencies)

Discover packages and versions:

- Registry UI: https://x07.io/packages
- Index catalog: https://registry.x07.io/index/catalog.json

Add a dependency:

```bash
x07 pkg add ext-base64-rs@0.1.2 --sync
```

Notes:

- `x07 pkg add` edits `x07.json`. With `--sync`, it also updates `x07.lock.json`.
- `x07 pkg lock` defaults to the official registry index when fetching is required; override with `--index` or forbid network with `--offline`.
- Official packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` will add and fetch these transitive deps automatically (and update `x07.json`).

See also: [Packages & projects](../packages/index.md).

## 7) World selection (determinism vs OS)

- Deterministic worlds: `solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`
- OS worlds: `run-os`, `run-os-sandboxed` (policy-gated, not a hardened sandbox)

Prefer profiles (`x07.json.default_profile` + `x07.json.profiles`) so agents usually run:

- `x07 run` (project default)
- `x07 run --profile os` / `x07 run --profile sandbox` (explicit intent)

See: [Fixture worlds](../worlds/fixture-worlds.md) and [OS worlds](../worlds/os-worlds.md).

## 8) Machine-first discovery surfaces

- CLI surface: `x07 --cli-specrows`
- Schemas: `spec/*.schema.json` (and the synced copies on x07lang.org under `/agent/.../schemas/`)
- External packages index: `GET /agent/latest/packages/index.json` on x07lang.org
- Local module inspection: `x07 doc <module-or-symbol>`
- Built-in reference guide: `x07 guide`

## 9) Known-good reference projects (copy/paste)

The `x07` repo ships three CI-gated example projects under `examples/agent-gate/`:

- `cli-newline` (pure CLI payload parsing)
- `cli-ext-cli` (CLI args via `ext-cli` + `argv_v1`)
- `web-crawler-local` (sandboxed OS networking + `--allow-host`, against a local fixture site)

See also: [Patch-based recipes](../recipes/index.md).
