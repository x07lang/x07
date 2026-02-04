# Agent quickstart (learn X07 from scratch)

This page is a single entry point for LLM agents. Use the published agent portal endpoints as the canonical source of schemas/skills/examples (see [Agent contracts](../agent/contract.md)).

## 0) The mental model

- X07 source is **x07AST JSON** (`*.x07.json`), not text.
- The toolchain is **JSON-first**: diagnostics, patches, and reports are structured.
- Programs run in worlds (fixture or OS); OS worlds are `run-os` and `run-os-sandboxed`.

High-level primitives to learn early (the “one whole system”):

- Streaming pipes: [`std.stream.pipe_v1`](../language/stream-pipes.md) (deterministic, budgeted streaming composition)
- Branded bytes: [`bytes@B` + `std.brand.*`](../language/types-memory.md#branded-bytes-typed-encodings) (typed boundary encodings)
- Structured concurrency: [`task.scope_v1`](../language/concurrency-multiprocessing.md#structured-concurrency-taskscopev1) (no orphan tasks; slots/select)
- Record/replay: [`std.rr` + rr scopes](../worlds/record-replay.md) (OS → deterministic cassette)
- Budget scopes: [`budget.scope_v1`](../language/budget-scopes.md) (localize cost contracts; arch-driven budgets)
- Contracts tooling: `x07 arch check`, `x07 schema derive`, `x07 sm gen` (pinned contracts → deterministic checks/generation)

## 1) Install and verify the toolchain

- Install with `x07up` (recommended).

Prerequisites (macOS / Linux):

- `sh` + `tar`
- `curl` or `wget` (used for downloads)
- a sha256 tool (`sha256sum`, `shasum`, or `openssl`) for installer verification
- for OS worlds / native deps (for example `ext-curl-c`): a C toolchain and libcurl headers (`x07 doctor` reports missing deps)

macOS / Linux (CI-safe, no profile edits, JSON report):

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- \
  --yes \
  --channel stable \
  --no-modify-path \
  --json
export PATH="$HOME/.x07/bin:$PATH"
```

If you are not in CI, you can omit `--no-modify-path` and the installer will attempt to persist the PATH change.

Verify:

```bash
x07 --help
x07 run --help
x07up show --json
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
x07 init
```

`x07 init` creates both the project skeleton and the agent kit:

- `x07.json` (with `os` and `sandbox` profiles)
- `x07.lock.json`
- `x07-toolchain.toml` (pins `stable` and declares `docs` + `skills`)
- `AGENT.md` (self-recovery guide + canonical commands)
- `.agent/docs/` (offline docs; linked to the installed toolchain when available)
- `.agent/skills/` (skills pack; linked to the installed toolchain when available)
- `src/` (a minimal program)
- `tests/tests.json` + `tests/smoke.x07.json` (a harness smoke test)

If you are creating a publishable package repo (for `x07 pkg publish`), use `x07 init --package` instead of `x07 init`.

See also: [Available skills](available-skills.md).

## 3) The core loop: run → test (auto-repair)

Run:

```bash
x07 run
```

`x07 run` runs the canonical auto-repair loop by default (format → lint → quickfix, repeatable). Use `--repair=off` when debugging, or `--repair=memory` to stage repairs without editing source files.

See: [Running programs](../toolchain/running-programs.md).

Diagnostics tip: many compiler errors include `ptr=/...` (a JSON Pointer into your x07AST) and sometimes `moved_ptr=/...` for ownership errors. Use `x07 ast get --in src/main.x07.json --ptr /...` to extract the failing node without manually counting indexes.

If you hit a compiler budget error (for example `Budget: max locals exceeded` or `Budget: AST too large`), see: [Compiler limits](../toolchain/compiler-limits.md).

Dependency tip: when compilation fails due to a missing module and prints `hint: x07 pkg add ... --sync`, `x07 run` can apply the hinted package add automatically and retry.

Run report tip: in `run-os`, the `rr_*` counters in `x07 run --report ...` are record/replay adapter stats and are not a count of real OS network requests.

If your program expects CLI arguments via `argv_v1`, pass them after `--` and `x07 run` will encode them into input bytes:

```bash
x07 run -- tool --help
```

Run the deterministic harness (repo-defined suites):

```bash
x07 test --manifest tests/tests.json
```

## 4) Repairs: quickfixes and patches

Two canonical repair mechanisms:

- **Quickfixes**: `x07 fix` applies the tool-provided `quickfix` JSON patches deterministically.
- **Explicit patches**: `x07 ast apply-patch` applies RFC 6902 JSON Patch.

Note: `x07 run`, `x07 build`, and `x07 bundle` apply quickfixes automatically by default (`--repair=...`). Use the explicit loop below when you want raw diagnostics or tighter control.

Example loop:

```bash
x07 lint --input src/main.x07.json
x07 fix --input src/main.x07.json --write
x07 lint --input src/main.x07.json
```

See: [Repair loop](../toolchain/repair-loop.md).

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
# Add the latest non-yanked version from the index:
x07 pkg add NAME --sync

# Or pin explicitly:
x07 pkg versions NAME
x07 pkg add NAME@VERSION --sync

# Remove a dependency:
x07 pkg remove NAME --sync
```

Notes:

- `x07 pkg add` edits `x07.json`. With `--sync`, it also updates `x07.lock.json`.
- If a module import fails and you don’t know which package provides it, use `x07 pkg provides <module-id>`.
- If you’ve added a package but don’t know which modules it exports, use `x07 doc <package-name>` (example: `x07 doc ext-net`).
- For stdlib modules, use `x07 doc std.<module>` (example: `x07 doc std.bytes`) and `x07 doc std.os.<module>` (example: `x07 doc std.os.env`).
- For builtins (example: `std.brand.*`), use `x07 guide`.
- For structured encodings, prefer branded bytes + validators over ad-hoc parsing (see `std.brand.cast_view_v1` / `std.brand.cast_view_copy_v1` in `x07 guide` and `meta.brands_v1` in schema-derived modules).
- For streaming transforms, prefer `std.stream.pipe_v1` and `std.io.bufread` over manual loops (more predictable allocations; fewer borrow/ownership hazards).
- `x07 pkg lock` defaults to the official registry index when fetching is required; override with `--index` or forbid network with `--offline`.
- Some packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` can add and fetch these transitive deps, but agents should treat the capability map + templates as canonical so the dependency set is explicit.
- Examples of transitive helpers: `ext-net` pulls `ext-curl-c`/`ext-sockets-c`/`ext-url-rs`, and `ext-db-sqlite` pulls `ext-db-core` (which pulls `ext-data-model`).

See also: [Packages & projects](../packages/index.md).

## 7) World selection (OS worlds)

- `run-os`: real OS adapters (disk/network/time/env/process)
- `run-os-sandboxed`: policy-gated OS adapters (not a hardened sandbox)

For `run-os-sandboxed`, start from a schema-valid policy template instead of hand-writing JSON:

```bash
x07 policy init --template worker --out policy/sandbox.base.json --mkdir-out
```

Prefer profiles (`x07.json.default_profile` + `x07.json.profiles`) so agents usually run:

- `x07 run` (project default)
- `x07 run --profile os` / `x07 run --profile sandbox` (explicit intent)

See: [OS worlds](../worlds/os-worlds.md).

## 8) Machine-first discovery surfaces

- CLI surface: `x07 --cli-specrows`
- Schemas: `spec/*.schema.json` (and the synced copies on x07lang.org under `/agent/.../schemas/`)
- External packages index: `GET /agent/latest/packages/index.json` on x07lang.org
- Offline docs: `x07up docs path --json`
- Local module inspection: `x07 doc <module-or-symbol>` (example: `x07 doc std.bytes`)
- Local package inspection (project deps): `x07 doc <package-name>` (example: `x07 doc ext-net`)
- Built-in reference guide: `x07 guide`

## 9) Known-good reference projects (copy/paste)

The `x07` repo ships CI-gated example projects under `examples/agent-gate/`:

- `cli-newline` (pure CLI payload parsing)
- `cli-ext-cli` (CLI args via `ext-cli` + `argv_v1`)
- `web-crawler-local` (sandboxed OS networking + `--allow-host`, against a local fixture site)

For the canonical package-focused set (examples + scenarios), see: [Agent workflow](agent-workflow.md).

See also: [Patch-based recipes](../recipes/index.md).

## 10) By-example tutorials (long-form)

- [Sandbox policy walkthrough](../worlds/sandbox-policy-walkthrough.md) (progressive policy + `--allow-host`)
- [Publishing by example](../packages/publishing-by-example.md) (author → test → pack → publish → consume)
- [Porting by example](../x07import/porting-by-example.md) (`x07import-cli` Rust/C → x07 package)
- [Testing by example](../toolchain/testing-by-example.md) (`x07 test` + fixtures)
