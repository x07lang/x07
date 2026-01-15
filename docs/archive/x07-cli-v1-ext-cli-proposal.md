# x07CLI v1 (external package) — `ext.cli.*` + `x07 cli` command surface

Status: **proposal** (repo-integrated, package-manager not required)

This document turns `docs/phases/x07-cli-proposals.md` plus the bundles in:

- `docs/phases/assets/x07_cli_v1_bundle.tar.gz`
- `docs/phases/assets/x07cli_semantic_and_bench_bundle.tar.gz`

into a concrete, implementable plan that fits the current repo reality:

- **No package manager yet** → package must be a local external package under `packages/ext/` using `x07-package.json`.
- Current standalone ABI is **solver-style** (stdin length prefix → `solve(bytes_view)->bytes` → stdout length prefix) → there is no direct `argv`/`stdout_write`/`stderr_write` surface in X07 code today without a larger ABI change.

As a result, v1 “CLI apps” are run via a toolchain wrapper (`x07 cli run`), while the CLI parser/library stays pure and deterministic.

---

## 0) Canonical sources and what we adopt

### We adopt (verbatim where possible)

From `x07_cli_v1_bundle.tar.gz`:

- `spec/x07cli.specrows.schema.json` (SpecRows schema)

From `x07cli_semantic_and_bench_bundle.tar.gz`:

- `scripts/check_x07cli_specrows_semantic.py` (semantic validator + canonicalizer)
- `docs/cli/cli-semantic-validator-v1.md` (normative semantic rules)
- `benchmarks/solve-pure/cli-v1-specrows-determinism.json` + `benchmarks/solve-pure/README_cli_v1.md` (input/output encodings + determinism assertions)

From `docs/phases/x07-cli-proposals.md`:

- the core “LLM-first” workflow: SpecRows → fmt/check → compile → parse → deterministic matches
- argv rules: `--`, `--opt=value`, `-abc` flag bundling, etc.

### We must reconcile (bundle mismatch)

The v1 bundle’s `docs/cli/cli-v1.md` describes **JSON matches output** via `result_bytes`, while the semantic bundle’s benchmark pins a **binary matches encoding**.

**Decision (v1):** the output format is the semantic bundle’s **binary matches_v1** (because it is already pinned by a deterministic suite). Any JSON-matches narrative is treated as outdated and must be updated if/when we import that file.

---

## 1) Repo integration: files and paths (A: `packages/ext/…` world)

### 1.1 Spec and tooling files (repo-root)

Add (from bundles):

- `spec/x07cli.specrows.schema.json`
- `scripts/check_x07cli_specrows_semantic.py`
- `docs/cli/cli-semantic-validator-v1.md`
- `benchmarks/solve-pure/cli-v1-specrows-determinism.json`
- `benchmarks/solve-pure/README_cli_v1.md`

### 1.2 External package (new)

Add:

- `packages/ext/x07-ext-cli/0.1.0/x07-package.json`
- `packages/ext/x07-ext-cli/0.1.0/modules/ext/cli.x07.json`
- `packages/ext/x07-ext-cli/0.1.0/modules/ext/cli/help.x07.json`
- `packages/ext/x07-ext-cli/0.1.0/modules/ext/cli/complete.x07.json`
- `packages/ext/x07-ext-cli/0.1.0/modules/ext/cli/tests.x07.json`
- `packages/ext/x07-ext-cli/0.1.0/tests/tests.json`

Module IDs are `ext.cli.*` (not `std.cli.*`) to avoid occupying the `std.*` namespace before a package manager / stdlib process exists.

### 1.3 Minimal dependency expectations (no transitive deps)

Because `x07-package.json` has no transitive dependencies, `ext.cli` must either:

1) be self-contained (includes its own SpecRows JSON parsing), or
2) import other already-shipped `ext.*` modules, and document that any project using `ext.cli` must also include those packages as direct dependencies.

**Proposal for v1:** option (2) is allowed, and we require:

- `ext-json-rs` (for `ext.json.data_model.parse` + canonicalization)
- `ext-data-model` (for traversing the parsed data model doc)
- `ext-unicode-rs` only if needed for strict UTF-8 rules (likely not needed v1)

These are already part of the supported external packages set.

---

## 2) End-user workflow (agent-friendly, deterministic)

Canonical project layout (example):

```
mytool/
  x07.json
  x07.lock.json
  cli/cli.specrows.json
  src/main.x07.json
```

Canonical steps:

1) Author `cli/cli.specrows.json` (SpecRows).
2) Canonicalize + implied defaults:
   - `x07 cli spec fmt --in cli/cli.specrows.json --write`
3) Validate:
   - `x07 cli spec check --in cli/cli.specrows.json --diag-out cli/cli.diag.json`
4) Run with real argv:
   - `x07 cli run --project x07.json -- --arg1 --flag`

Notes:

- The `--` after `x07 cli run` separates wrapper flags from app argv, and is *not* the same as the app-level `--` delimiter (which is inside argv_v1).
- This keeps the app-level parsing deterministic: `argv_v1` bytes are always built the same way.

---

## 3) SpecRows v1 (authoring + canonicalization)

### 3.1 Schema

Normative schema file: `spec/x07cli.specrows.schema.json`.

Row kinds (schema + semantic doc):

- `about`: `[scope,"about",desc]`
- `help`: `[scope,"help",shortOpt,longOpt,desc]`
- `version`: `[scope,"version",shortOpt,longOpt,desc]` (root only implied)
- `flag`: `[scope,"flag",shortOpt,longOpt,key,desc,(meta?)]`
- `opt`: `[scope,"opt",shortOpt,longOpt,key,value_kind,desc,(meta?)]`
- `arg`: `[scope,"arg",POS_NAME,key,desc,(meta?)]`

### 3.2 Scope rules (v1)

v1 supports **root + 1-level subcommands**:

- `scope == "root"` means the root command.
- any other scope name creates a subcommand with that name.

### 3.3 Canonicalization / implied defaults

Normative: `docs/cli/cli-semantic-validator-v1.md` and `scripts/check_x07cli_specrows_semantic.py`.

Key v1 rules:

- deterministic diagnostics ordering
- per-scope canonical row ordering (about, help, version, flags, opts, args)
- implied defaults:
  - help is implied for every scope (`-h/--help` or `--help` if `-h` taken)
  - version is implied for root (`-V/--version` or `--version` if `-V` taken)

---

## 4) argv_v1 (the one canonical input)

Normative encoding for argv blob (`argv_v1`), used both in solve-* tests and wrapper runs:

```
u32_le argc
repeat argc:
  u32_le len
  len bytes (UTF-8 token bytes, no NUL)
```

`argv[0]` is included if the wrapper has it; parsers must not require it.

---

## 5) Parse output contract: `matches_v1` / `err_v1` (binary)

### 5.1 Success (`matches_v1`)

This is pinned by `benchmarks/solve-pure/README_cli_v1.md` and `benchmarks/solve-pure/cli-v1-specrows-determinism.json`.

Success layout:

```
u8  tag = 1
u32_le cmd_len
cmd bytes (UTF-8 scope string, e.g. "root" or "serve")
u32_le entry_count
repeat entry_count (sorted by key bytes ascending):
  u32_le key_len
  key bytes (UTF-8)
  u8  kind (1=flag, 2=opt, 3=arg, 4=multi)
  u32_le value_len
  value bytes
```

Rules:

- entries are sorted by raw key bytes ascending
- for flags, `value bytes` is exactly one byte `u8(count)` (so one occurrence is `0x01`)
- for `kind=2` (opt) and `kind=3` (arg), `value bytes` are the captured token bytes (exact substring for `--opt=value` per parser rules)
- for `kind=4` (multi), `value bytes` are encoded as:
  - `u32_le count`
  - repeated `count` times:
    - `u32_le len`
    - `len` bytes

### 5.2 Error (`err_v1`)

This is the missing half that makes the library usable in real apps.
It follows the early contract in `docs/phases/x07-cli-proposals.md` (tagged ok/err), while keeping the success half pinned by the existing suite.

Error layout:

```
u8  tag = 0
u32_le code
u32_le msg_len
msg bytes (UTF-8, stable, no host paths)
u32_le usage_len
usage bytes (UTF-8, stable, no wrapping by terminal width)
```

Error code catalog (v1 minimal; aligns with the v1 bundle’s numeric guidance):

- `1001` invalid spec (schema/semantic)
- `1002` usage error (unknown option, missing required, arity error)
- `1003` bad value (failed `value_kind` parse/validation)
- `1099` internal error (bug)

Exit code policy is handled by the wrapper (`x07 cli run`), not by the pure parser.

---

## 6) Parser semantics (v1)

Normative intent: `docs/phases/x07-cli-proposals.md` + semantic validator rules.

Required v1 behavior:

- `--` ends option parsing; remaining tokens are positionals/rest.
- Long options:
  - `--name`
  - `--name=value`
  - `--name value`
- Short options:
  - `-a` flag
  - short bundles like `-abc` **only for flags**
  - short option values: `-ovalue` and `-o value`
- Subcommand selection:
  - first non-option token matching a declared subcommand selects it
  - v1 supports only one subcommand level
- Duplicate policy:
  - flags: count occurrences, encode as 1 byte `u8(count)` (saturate at 255)
  - opts:
    - `multiple=false` (default): last-wins (`kind=2`)
    - `multiple=true`: collect (`kind=4`)
  - args:
    - ordered binding
    - `multiple=true` only allowed on final arg and uses `kind=4`

Value kinds (schema):

- `STR`, `PATH`, `U32`, `I32`, `BYTES`, `BYTES_HEX`

---

## 7) `ext.cli.*` API surface (modules and exports)

### 7.1 `ext.cli` (pure)

Exports:

- `ext.cli.specrows.validate(spec_json: bytes) -> result_i32`
- `ext.cli.specrows.compile(spec_json: bytes) -> result_bytes` (specbin_v1)
- `ext.cli.parse_compiled(specbin_v1: bytes, argv_v1: bytes) -> bytes` (`matches_v1`/`err_v1`)
- `ext.cli.parse_specrows(spec_json: bytes, argv_v1: bytes) -> bytes` (compile+parse)

Accessors (pure helpers; recommended so agents never parse blobs by hand):

- `ext.cli.is_ok(doc: bytes_view) -> i32`
- `ext.cli.err_code(doc: bytes_view) -> i32`
- `ext.cli.err_msg(doc: bytes_view) -> bytes`
- `ext.cli.err_usage(doc: bytes_view) -> bytes`
- `ext.cli.matches_cmd(doc: bytes_view) -> bytes`
- `ext.cli.matches_get(doc: bytes_view, key: bytes_view, kind: i32) -> option_bytes`

### 7.2 `ext.cli.help` / `ext.cli.complete` (pure)

Exports:

- `ext.cli.help.render(spec_json_or_specbin: bytes) -> bytes` (v1: stable text, no terminal-width wrapping)
- `ext.cli.complete.render(spec_json_or_specbin: bytes, shell: bytes) -> bytes` (`bash|zsh|fish|powershell`)

v1 can start with spec_json input and later switch to specbin once stable.

---

## 8) Toolchain command surface: `x07 cli …` (exact subcommands)

These commands are for external users (agents) and must be deterministic and machine-friendly.

### 8.1 `x07 cli spec fmt`

```
x07 cli spec fmt --in <PATH> [--out <PATH>] [--write]
```

- Reads `SpecRows` JSON
- Validates JSON Schema (`spec/x07cli.specrows.schema.json`)
- Applies semantic canonicalization + implied defaults (same as `scripts/check_x07cli_specrows_semantic.py fmt`)
- Output:
  - if `--write`: overwrites `--in`
  - else writes canonical JSON to `--out` (default `-` = stdout)
- Exit codes:
  - `0` ok
  - `2` invalid invocation (bad flags)
  - `20` schema/semantic failure

### 8.2 `x07 cli spec check`

```
x07 cli spec check --in <PATH> [--diag-out <PATH>]
```

- Reads and schema-validates `SpecRows`
- Runs semantic validation (same rules/codes as `scripts/check_x07cli_specrows_semantic.py check`)
- Emits a JSON report to stdout:
  - `ok: bool`
  - `in: string`
  - `diagnostics_count: number`
- If `--diag-out` is provided, writes the full diagnostics list (stable ordering) to that path.

### 8.3 `x07 cli spec compile`

```
x07 cli spec compile --in <PATH> --out <PATH>
```

- Reads and schema-validates spec
- Canonicalizes + implied defaults
- Compiles to `specbin_v1` bytes (as produced by `ext.cli.specrows.compile`)
- Writes raw bytes to `--out`

### 8.4 `x07 cli run`

```
x07 cli run --project <x07.json> [--world <solve-pure|solve-fs|run-os|run-os-sandboxed>] -- <argv...>
```

- Encodes `<argv...>` into `argv_v1`
- Runs the project’s entry with input = `argv_v1` bytes via the appropriate runner
- Interprets the returned bytes as:
  - either application-defined (v1 minimal), or
  - a future standardized “run-result” record (v1.1+)
- Exit codes:
  - `0` program ok
  - `2` CLI usage error (if the program returns `err_v1` / `--help` path)
  - `1` runtime failure (trap, compile failure, etc)

This is the v1 replacement for a missing `argv`/`stdout_write` ABI in X07 itself.

---

## 9) Acceptance tests (repo-integrated)

### 9.1 Schema + semantic validator

- Add at least one example spec (minimal + subcommand) and run:
  - `python3 scripts/check_x07cli_specrows_semantic.py check <spec>`
  - `python3 scripts/check_x07cli_specrows_semantic.py fmt <spec> --in-place`

### 9.2 Deterministic behavior (pure)

Adopt the semantic bundle’s suite and ensure it runs in CI. This requires one internal tooling addition:

- Add `--module-root <DIR>` (repeatable) to `x07-host-runner` and plumb it into `compile_options.module_roots`.
- Extend `scripts/bench/run_bench_suite.py` to pass module roots via an env var (proposal):
  - `X07_BENCH_MODULE_ROOT=stdlib/std/0.1.1/modules:packages/ext/x07-ext-cli/0.1.0/modules:…`

Then:

- Add reference solution:
  - `benchmarks/solutions/cli/specrows_compile_parse_v1.x07.json`
  - imports `ext.cli` and implements exactly the input/output contract in `benchmarks/solve-pure/README_cli_v1.md`.
- Run:
  - `python3 scripts/bench/run_bench_suite.py --suite benchmarks/solve-pure/cli-v1-specrows-determinism.json --solutions benchmarks/solutions`

### 9.3 Package unit tests (`x07 test`)

Inside `packages/ext/x07-ext-cli/0.1.0/tests/tests.json`, add tests covering:

- implied help insertion (`--help` works even if not specified)
- `--` delimiter behavior
- `--opt=value` vs `--opt value`
- `-abc` bundling (flags only)
- required arg constraints

Run via:

- `cargo run -p x07 -- test --manifest packages/ext/x07-ext-cli/0.1.0/tests/tests.json --module-root <root>`

### 9.4 Canary/CI wiring

Add to `scripts/ci/check_canaries.sh` after existing suites:

- `python3 scripts/bench/run_bench_suite.py --suite benchmarks/solve-pure/cli-v1-specrows-determinism.json --solutions "$solutions_dir"`

---

## 10) Explicit non-goals (v1)

- Nested subcommands (only root + 1 level).
- Rich TUI/colored help or terminal-width wrapping.
- OS-native argv/stdout/stderr without the `x07 cli run` wrapper (requires ABI work).
