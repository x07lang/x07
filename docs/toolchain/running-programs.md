# Running programs (canonical): `x07 run`

`x07 run` is the single front door for executing X07 programs. It selects the appropriate runner based on the chosen world and prints a machine-readable JSON report.

## Choose what to run

Exactly one target mode is used:

- `x07 run` (no flags): auto-discovers a project manifest (`x07.json` or a single `*.x07project.json`) by walking up from the current directory.
- `x07 run --project path/to/x07.json`: run a specific project manifest (+ lockfile).
- `x07 run --program path/to/main.x07.json`: compile+run a single file (module roots are inferred unless `--module-root` is provided).
- `x07 run --artifact path/to/exe`: run a precompiled executable (runner output only; no project metadata).

## Executable I/O framing (advanced)

Executables produced by the X07 runners use a simple byte framing:

- stdin: `u32_le(len)` then `len` bytes
- stdout: `u32_le(len)` then `len` bytes
- stderr: JSON stats

`x07 run`, `x07-host-runner`, and `x07-os-runner` handle this framing automatically.

This “framed I/O” executable ABI is designed for machine-driven execution. If you want a normal end-user CLI binary (no framing, standard `argc/argv`, raw stdout) use `x07 bundle`.

## Pick a world (deterministic vs OS)

Worlds:

- Deterministic evaluation worlds: `solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`
- OS worlds: `run-os`, `run-os-sandboxed` (policy-gated; not a hardened sandbox)

Recommended: define intent-driven profiles in your project (`x07.json.default_profile` + `x07.json.profiles`) and use:

- `x07 run` (uses `default_profile`)
- `x07 run --profile os`
- `x07 run --profile sandbox`

World resolution precedence is:

1. `--world ...`
2. `--os` / `--host`
3. `--profile ...` (or `default_profile`)
4. project manifest `world`
5. default: `solve-pure`

## Runner selection

By default, `x07 run` chooses the runner from the selected world:

- `solve-*` → `x07-host-runner`
- `run-os*` → `x07-os-runner`

You can override this with:

- `--runner auto|host|os`
- `--host` / `--os` (shorthand; also affects world selection if `--world` is not set)

The host runner is only valid for `solve-*` worlds, and the OS runner is only valid for `run-os*` worlds.

## Provide input bytes

Input selection (highest precedence first):

1. `--input path/to/input.bin`
2. `--stdin` (reads all stdin bytes)
3. `--input-b64 <BASE64>`
4. trailing args after `--` (encoded as `argv_v1`)
5. `profiles.<name>.input` (project-relative path)
6. no input (empty bytes)

Example: pass CLI-style args via `argv_v1`:

```bash
x07 run --profile os -- tool --help
```

## Fixtures (solve-fs / solve-rr / solve-kv / solve-full)

Fixture worlds require fixture directories. `x07 run` resolves them in this order:

1. per-world flags like `--fixture-fs-dir`, `--fixture-rr-dir`, `--fixture-kv-dir`
2. `--fixtures <dir>` (expands to `<dir>/fs`, `<dir>/rr`, `<dir>/kv` if they exist)
3. under the project root: `.x07/fixtures/<kind>/` or `fixtures/<kind>/`

Required fixture dirs:

- `solve-fs` requires an `fs` fixture dir
- `solve-rr` requires an `rr` fixture dir
- `solve-kv` requires a `kv` fixture dir
- `solve-full` requires all three

See: [Fixture formats](../worlds/fixture-formats.md).

## Policies (run-os-sandboxed)

`run-os-sandboxed` requires a base policy file:

- via profile: `profiles.sandbox.policy`
- or via CLI: `x07 run --world run-os-sandboxed --policy path/to/policy.json`

To generate a schema-valid starting policy, use `x07 policy init --template <...>` and then extend it for your app.

Convenience overrides for net/filesystem allowlists are available only in `run-os-sandboxed` and materialize a derived policy under `.x07/policies/_generated/`:

- `--allow-host host:port[,port...]` / `--deny-host host:*|host:port[,port...]` (deny wins)
- `--allow-read-root /abs/or/rel/path` / `--allow-write-root /abs/or/rel/path`

Note: `--allow-host` requires explicit ports; `*` is accepted only by `--deny-host`.

## Compilation and resource knobs

Common flags:

- `--cc-profile default|size`
- `--max-c-bytes <BYTES>`
- `--compiled-out <PATH>` (write the produced runner executable; framed I/O)
- `--compile-only` (deterministic `solve-*` worlds only; not valid with `--artifact`)
- `--solve-fuel <N>`, `--max-memory-bytes <N>`, `--max-output-bytes <N>`, `--cpu-time-limit-seconds <N>`
- `--auto-ffi` / `--no-auto-ffi` (OS worlds; collect/apply C FFI flags from dependencies)

## Distribute a native executable (canonical): `x07 bundle`

`x07 bundle` produces a native CLI executable that can be run directly (no framed I/O) and does not require the X07 toolchain at runtime.

Examples:

```bash
# Bundle an OS-world CLI binary:
x07 bundle --profile os --out dist/mytool
./dist/mytool --help

# Bundle a policy-enforced OS-world binary (requires a base policy via profile or --policy):
x07 bundle --profile sandbox --out dist/mytool
```

The bundled binary encodes `argc/argv` to `argv_v1` input bytes and writes the program output bytes directly to stdout.

## Reports (stdout + optional file)

`x07 run` always prints a JSON report to stdout. You can also write the same bytes to a file with `--report-out <PATH>`.

Report modes:

- `--report runner` (default): pass through the runner JSON (`spec/x07-host-runner.report.schema.json` or `spec/x07-os-runner.report.schema.json`)
- `--report wrapped`: wrap the runner JSON in `spec/x07-run.report.schema.json` (`schema_version: "x07.run.report@0.1.0"`)

### Common failure: unknown module (missing deps)

If compilation fails because a module can’t be resolved, the report will have:

- `compile.ok: false`
- `compile.compile_error` containing something like `Parse: unknown module: "ext.cli" (searched: ext/cli.x07.json)`

Fix:

- for packages: add the dependency (`x07 pkg add <name>@<ver> --sync`) so `x07.lock.json` provides module roots, or
- for standalone `--program` runs: ensure the module exists under a `--module-root` directory.
