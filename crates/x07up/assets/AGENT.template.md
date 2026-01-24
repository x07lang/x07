# AGENT.md — X07 Agent Operating Guide (Self-Recovery)

This repository is an X07 project. You are a coding agent. Your job is to make changes *and* autonomously recover from errors using the X07 toolchain and its JSON contracts.

## Canonical entrypoints (do not guess)
- Build / format / lint / fix: `x07 fmt`, `x07 lint`, `x07 fix`
- Run: `x07 run` (single front door; emits JSON reports)
- Test: `x07 test` (JSON report; deterministic suites)
- Policies: `x07 policy init` and `x07 run --allow-host/--deny-host/...` (derived policy generation)
- Packages: `x07 pkg add`, `x07 pkg lock` (prefer combined flows when available)

Avoid calling low-level binaries directly (`x07c`, `x07-host-runner`, `x07-os-runner`) unless the task explicitly requires “expert mode”.

## Toolchain info (fill by x07up)
- Toolchain: {{X07_TOOLCHAIN_VERSION}}
- Installer channel: {{X07_CHANNEL}}
- Docs root: {{X07_DOCS_ROOT}}
- Skills root: {{X07_SKILLS_ROOT}}

If any of the above are missing, run:
- `x07up show --json`
- `x07up doctor --json`

## Standard recovery loop (run this, in order)
When something fails (compile/run/test), follow this loop *without asking for help first*:

1) Format:
- `x07 fmt --write <files...>`

2) Lint:
- `x07 lint --json <files...> > .x07/lint.last.json || true`

3) Quickfix:
- `x07 fix --write <files...> --json > .x07/fix.last.json || true`

4) Re-run the failing command with a wrapped report:
- `x07 run --report wrapped --report-out .x07/run.last.json ...`
- or `x07 test --json --report-out .x07/test.last.json ...`

5) If still failing, inspect the JSON report fields first (not stdout):
- Look for `ok: false`, `compile_error`, `trap`, and `stderr_b64`.
- Decode base64 payloads deterministically (example below).

Only after (1)-(5) should you change code again.

## Decoding base64 fields (copy/paste)
Many runner reports use base64 fields for binary outputs. Use this exact snippet:

```bash
python3 - <<'PY'
import base64, json, sys
p = ".x07/run.last.json"
doc = json.load(open(p, "r", encoding="utf-8"))
r = doc.get("report") if doc.get("schema_version","").startswith("x07.run.report@") else doc
for k in ("stderr_b64","stdout_b64","solve_output_b64"):
    if k in r and isinstance(r[k], str):
        raw = base64.b64decode(r[k])
        print(f"{k}: {len(raw)} bytes")
        try:
            print(raw.decode("utf-8", errors="replace")[:2000])
        except Exception:
            pass
PY
```

## Project execution model (must be consistent)

* `x07.json` defines:

  * `default_profile`
  * `profiles.<name>` with `world`, optional `policy`, optional resource limits
* `x07.lock.json` (or configured lockfile) defines resolved package module roots.
* `x07 run --profile <name>` is the canonical way to select world/policy.

Do **not** pass long lists of `--module-root` manually unless in “expert mode”. The project lockfile must resolve them.

## Worlds: operational rule of thumb

* Use **`solve-*`** worlds for deterministic logic and unit tests.
* Use **`run-os`** for “real” apps (network/FS/process), when sandboxing is not required.
* Use **`run-os-sandboxed`** for controlled execution with an explicit policy file.

If a task looks “real world” (CLI tool, HTTP client, web service), default to `run-os-sandboxed` + a base policy template, then widen intentionally.

## Policy workflow (do not hand-edit derived policies)

* Base policies live in: `.x07/policies/base/`
* Generated/derived policies live in: `.x07/policies/_generated/` (do not commit; do not edit)

Generate a base policy:

* `x07 policy init --template cli --out .x07/policies/base/cli.sandbox.base.policy.json`

Run with a derived policy:

* `x07 run --profile sandbox --allow-host example.com:443 --report wrapped --report-out .x07/run.last.json`

## Help surfaces (canonical)

* `x07 --help`
* `x07 run --help`
* `x07 pkg --help`
* `x07 test --help`
* `x07 policy init --help`
* `x07 fmt --help`
* `x07 lint --help`
* `x07 fix --help`

From a temp project:

* `x07 init`
* `x07 run --profile test --stdin --report wrapped --report-out .x07/run.last.json`

## “Expert tools” (only if explicitly asked)

* `x07c` (compiler)
* `x07-host-runner` (host runner)
* `x07-os-runner` (OS runner)

If you must use them, preserve the report JSON and include the exact invocation.

