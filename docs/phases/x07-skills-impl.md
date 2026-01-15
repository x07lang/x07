Status: implemented (end-user skills pack v0.1.0)

Source of truth:

- Skills pack content: `skills/pack/.codex/skills/` (install target: `.codex/skills/`)
- Release artifact: `dist/x07-skills-<tag>.tar.gz` (built by `scripts/build_skills_pack.py`)
- Validation: `scripts/check_skills.py` (runs via `./scripts/ci/check_skills.sh` and `./scripts/ci/check_all.sh`)

Constraints:

- Skills are for end-users/coding agents writing X07 programs (toolchain-only).
- Skills must not depend on the toolchain source repo paths (no `docs/`, no `scripts/`, no `cargo`).

Included skills:

- `x07-agent-playbook`
- `x07-build-run`
- `x07-lint-repair`
- `x07-format`
- `x07-package`
- `x07-test`
- `x07-os-run`
- `x07-ffi-c`
- `x07-io-streams`
- `x07-concurrency`

++++

## Archived design notes

The remainder of this file is historical context and may not match the implemented skills pack layout.


Here’s a **production‑oriented “X07 Skills” design** that’s compatible with the open Agent Skills pattern (Codex / Claude Code / Copilot style), but **does not depend on any compile-time macro system or outer-loop tuning tooling**—it’s aimed at **end‑users’ autonomous coding agents** building and maintaining X07 applications.

---

## What “skills” are and why they help agentic coding

Across Codex/Claude-style agents, a “skill” is intentionally simple:

* A **directory** containing a `SKILL.md` file
* `SKILL.md` starts with **YAML frontmatter** (`name`, `description`)
* Optional folders like `scripts/`, `references/`, `assets/`
* **Progressive disclosure**: the agent loads only metadata at startup, then loads full instructions and any referenced files only when needed. ([OpenAI Developers][1])

Codex also standardizes **where skills live** (repo‑scoped `.codex/skills`, user `~/.codex/skills`, etc.). ([OpenAI Developers][1])

**Why this matters for X07:** Skills let you encode “how we do things” as a repeatable workflow (compile → lint → apply patch → re-run tests), which is exactly what an autonomous agent needs when syntax is strict and errors are frequent.

---

## Key design decision: “X07 Skills Pack” is a product artifact

Because you want **standalone / production** (no overlays/benchmarks), treat skills as a **shippable, versioned companion** to the X07 toolchain.

### Two-tier skill distribution

1. **Toolchain-bundled skills (recommended)**

* Shipped with X07 releases, installed into the user’s global skill dir (e.g. `~/.codex/skills`) or copied into a project.
* Benefit: every user gets a consistent baseline.

2. **Project skills (optional)**

* Checked into an app repo under `.codex/skills/` so the agent gets project-specific policies (coding style, module layout, CI commands, etc.). ([OpenAI Developers][1])

This mirrors how Codex scopes skills by location and precedence. ([OpenAI Developers][1])

---

## Proposed repo layout for production skills

### Canonical on-disk location (repo-scoped skills)

Codex discovers repo skills under `$REPO_ROOT/.codex/skills/`.

```
.codex/skills/
  README.md
  x07-build-run/
    SKILL.md
    scripts/
      build_run.py
      parse_run_output.py
    references/
      cli.md
  x07-lint-repair/
    SKILL.md
    scripts/
      lint_json.py
      apply_patch.py
      repair_loop.py
    assets/
      repair_prompt_template.md
    references/
      x07diag.md
      json_patch.md
  x07-format/
    SKILL.md
    scripts/
      format.py
  x07-test/
    SKILL.md
    scripts/
      test.py
      snapshot_update.py
  x07-package/
    SKILL.md
    scripts/
      add_dep.py
      lock_verify.py
    references/
      manifest.md
      lockfile.md
  x07-io-streams/
    SKILL.md
    references/
      std.io.md
  x07-concurrency/
    SKILL.md
    references/
      async.md
  x07-ffi-c/
    SKILL.md
    scripts/
      gen_header.py
    references/
      ffi.md
  x07-os-run/
    SKILL.md
    scripts/
      run_os.py
      sandbox_policy_check.py
    references/
      sandbox_policy.md
```

### Why this structure works well with agents

* It matches the expected “a folder with `SKILL.md` + optional scripts/assets/references” pattern. ([OpenAI Developers][1])
* It leverages progressive disclosure: keep `SKILL.md` short and link deeper reference docs and scripts (agents can read only what they need). ([Anthropic][2])
* Scripts provide deterministic operations (validation, patch applying, lockfile updates), which is explicitly called out as a major benefit of skills. ([Anthropic][2])

---

## Naming conventions that avoid cross-agent incompatibilities

Different agent environments can be pickier about naming. A safe choice:

* `name`: lowercase kebab-case, unique, <= 100 chars
* `description`: one line, <= 500 chars
* Put **extra metadata** under `metadata:` (Codex ignores extra keys, so you can store version constraints). ([OpenAI Developers][3])

Some ecosystems explicitly require lowercase/hyphen naming; adopting it makes the skills pack more portable. ([GitHub Docs][4])

Example header:

```yaml
---
name: x07-lint-repair
description: Run X07 lint/check in JSON mode and apply deterministic JSON Patch repairs until clean or blocked.
metadata:
  x07_min: "0.2.0"
  tools: ["x07c", "x07lint", "x07fmt"]
---
```

---

## The “must-have” production skills for 100% agentic X07 coding

Below is the smallest set that makes autonomous coding robust. Each one is a **workflow** with deterministic scripts and strict input/output expectations.

### 1) `x07-build-run`

**Purpose:** compile and run a target program, capture structured outputs.

**Agent value:** avoids “I think it compiled” failures; gives a canonical way to run.

**Scripts to include:**

* `build_run.py`: runs compiler with standardized flags, runs executable, captures stdout/stderr, emits JSON summary.

### 2) `x07-lint-repair` (highest leverage)

**Purpose:** turn compiler/linter diagnostics into deterministic repairs.

**Agent value:** this is the foundation for “self-healing” code generation.

**Core workflow:**

1. Run `x07 lint --format json` (or equivalent).
2. If errors: produce a JSON Patch following your patch schema.
3. Apply patch using `apply_patch.py` (deterministic).
4. Re-run lint/check until clean or “needs human decision”.

This matches the “skills can bundle scripts to execute deterministic processing” advantage. ([Anthropic][2])

### 3) `x07-format`

**Purpose:** enforce canonical formatting / stable ordering.

**Agent value:** prevents diffs from exploding; makes repair patches stable.

### 4) `x07-test`

**Purpose:** run unit tests and “fixture replay” tests in a stable way (not your eval harness—just normal product testing).

**Agent value:** prevents regressions; closes the loop for agent autonomy.

### 5) `x07-package`

**Purpose:** manage dependencies + lockfile deterministically.

**Agent value:** prevents dependency drift; makes builds reproducible.

### 6) `x07-os-run`

**Purpose:** run programs in OS world / sandboxed OS world with policy enforcement.

**Agent value:** enables real-world usage while keeping safe defaults (very important for autonomous agents).

### 7) `x07-ffi-c`

**Purpose:** create C FFI bindings safely (headers, symbol lists, link flags).

**Agent value:** unlocks the “C/Rust-class” ecosystem story.

---

## How to keep skills useful as X07 grows (without bloating SKILL.md)

This is the biggest mistake people make: stuffing the entire language guide into every skill.

Instead:

### Progressive disclosure rules (enforceable)

* **SKILL.md body is an index + workflow**, not a textbook.
* Anything large goes into `references/*.md` and is loaded only when needed. ([Anthropic][2])
* Any deterministic operation goes into `scripts/` (the agent runs scripts instead of re-generating logic). ([Anthropic][2])

This is aligned with how Codex and Anthropic describe skills scaling: metadata first, then skill body, then linked files. ([OpenAI Developers][1])

### Skill “contract blocks”

In each `SKILL.md`, include small, rigid sections:

* **Inputs** (what files/paths are expected)
* **Outputs** (JSON result shape, patch shape)
* **Allowed tools/commands**
* **Failure modes** (what to do if repair doesn’t converge)
* **Examples** (1–2 minimal “golden” examples)

This keeps SKILL.md short but operational.

---

## How to ensure skills stay production-grade: enforcement and CI gates

Even though this is “end-user tooling”, you still want CI on the skills pack repo so it doesn’t rot.

### Add a skills linter (repo script)

`scripts/check_skills.py` should:

* Find every `*/SKILL.md`
* Parse YAML frontmatter
* Validate:

  * required `name`, `description` ([OpenAI Developers][3])
  * name format (choose strict: lowercase kebab-case)
  * unique names across the repo
* Verify all referenced `scripts/` / `references/` files exist
* Optionally enforce “SKILL.md max length” (to preserve progressive disclosure)

### Add smoke tests for scripts

For each script, run it against a tiny example project (in `tests/fixtures/`).

---

## How end-users install/use these skills

For Codex specifically, document the repo-scoped install path:

* Project: `<repo>/.codex/skills/<skill-name>/SKILL.md`
* User: `~/.codex/skills/<skill-name>/SKILL.md` ([OpenAI Developers][1])

Your installer can simply copy skill directories into those locations.

Because the format is open and simple (`SKILL.md` + folders), the same pack can be used by other compatible agents as well (Claude Code, Copilot-style environments). ([Anthropic][2])

---

## Practical “first implementation” development plan

### Milestone 1: Define the baseline contracts

Deliver:

* `references/cli.md`: canonical X07 CLI commands
* `references/x07diag.md`: diagnostic JSON schema (or link to your existing schema files)
* `references/json_patch.md`: patch format + examples

### Milestone 2: Implement the 4 core skills

* `x07-build-run`
* `x07-lint-repair`
* `x07-format`
* `x07-test`

### Milestone 3: Add dependency + OS skills

* `x07-package`
* `x07-os-run`

### Milestone 4: Add ecosystem unlockers

* `x07-ffi-c`
* optional `x07-docgen` (API docs generation)

### Milestone 5: Add CI gates

* `scripts/check_skills.py`
* `tests/` smoke runner

---

## The main “agentic reliability” trick: make skills *script-first*

If X07 is strict and the agent struggles with syntax, you don’t want skills that say “write code like this…”.

You want skills that say:

1. Run `x07lint --json`
2. Read diagnostics
3. Apply a **machine-checked patch**
4. Re-run `x07lint --json`
5. Only if clean, run tests
6. Only if tests pass, open PR

This is exactly the kind of deterministic “do X, then verify” workflow skills are meant to encode, and why bundling scripts is emphasized in skills guidance. ([Anthropic][2])

---
[1]: https://developers.openai.com/codex/skills/ "Agent Skills"
[2]: https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills "Equipping agents for the real world with Agent Skills \ Anthropic"
[3]: https://developers.openai.com/codex/skills/create-skill/ "Create skills"
[4]: https://docs.github.com/copilot/concepts/agents/about-agent-skills?utm_source=chatgpt.com "About Agent Skills"

+++++
Seed archive (historical): `docs/phases/assets/x07_skills_repo_scoped.tar.gz` (contains `evolang-*` skills).
Canonical X07 skills are checked into this repo under `.codex/skills/x07-*`.
Source of truth: read the checked-in `.codex/skills/<skill>/SKILL.md` (and any referenced scripts) rather than relying on inlined examples in this doc.

This archive extracts to **`.codex/skills/...`**, which is one of Codex’s supported skill locations. ([OpenAI Developers][1])
(Anthropic’s “Agent Skills” concept is also the same “folder with `SKILL.md` (YAML frontmatter) + optional scripts/resources” pattern, so these skills are portable in spirit. ([Anthropic][2]))

## Where to put these skills

Canonical layout in the repo root:

```
.codex/skills/
  x07-agent-playbook/
  x07-lint-repair/
  x07-build-run/
  x07-package/
```

Codex will discover repo skills under `$REPO_ROOT/.codex/skills`. ([OpenAI Developers][1])

---

## 1) Skill: x07-lint-repair

### Files

```
.codex/skills/x07-lint-repair/
  SKILL.md
  scripts/
    lint_repair.py
```

### `.codex/skills/x07-lint-repair/SKILL.md`

````md
---
name: x07-lint-repair
description: Lint X07 x07AST JSON files, emit machine-readable diagnostics, and apply JSON Patch repairs deterministically (for autonomous agentic workflows).
metadata:
  short-description: Lint + apply JSON Patch repairs for X07
  version: 0.1.0
  kind: script-backed
---

# x07-lint-repair

This skill is a **deterministic** wrapper around X07's linter plus a safe patch-apply loop.
It is designed for **autonomous coding agents** that generate X07 programs (x07AST JSON) and need a reliable way to:
1) get structured diagnostics, and
2) apply a proposed repair patch, and
3) re-lint until clean.

## When to use

Use this skill when:
- an X07 file fails lint/compile due to syntax/arity/type errors,
- you want the agent to "self-repair" by iterating: **lint → patch → lint**,
- you want CI-friendly, machine-readable outputs (no prose).

## Inputs

- `--file PATH`: Path to an X07 program/module in **x07AST JSON** form (`*.x07.json`).
- `--apply-patch PATCH.json` (optional): JSON Patch (RFC 6902) to apply to the x07AST file.
- `--in-place` (optional): Write patched content back to `--file`.

## Outputs

The script prints a **single JSON object** to stdout:

```json
{
  "ok": true,
  "lint_exit_code": 0,
  "diagnostics": { "...": "x07Diag JSON" },
  "applied_patch": false,
  "files_modified": []
}
````

* If lint fails or `x07c` is missing, `ok=false` and `diagnostics` contains a deterministic error payload.

## Workflow (agent loop)

1. Lint:

   * `python3 scripts/lint_repair.py --file src/main.x07.json`

2. If `ok=false` and diagnostics contain errors, the agent produces a **JSON Patch** that fixes *only* the reported issues.

3. Apply + re-lint:

   * `python3 scripts/lint_repair.py --file src/main.x07.json --apply-patch /tmp/repair.patch.json --in-place`

4. Repeat (max 3 iterations). If still failing:

   * perform a small, targeted rewrite (keep diffs minimal),
   * or regenerate the x07AST cleanly from scratch.

## Hard rules (to keep repairs safe)

* Do **not** apply “broad refactors” as a repair. Keep patches minimal.
* Prefer **replace/remove/add** operations on the smallest subtrees needed.
* Never introduce new dependencies during repair.
* If the file is not valid JSON, do not guess: rewrite into valid JSON first.

## Files

* `scripts/lint_repair.py` — the deterministic wrapper + JSON Patch applier.

````

### `.codex/skills/x07-lint-repair/scripts/lint_repair.py`

```python
#!/usr/bin/env python3
"""
Deterministic lint + JSON-Patch apply wrapper for X07 x07AST JSON files.

Design goals:
- No network.
- Stable JSON output (single object printed to stdout).
- Minimal patch operations supported: add/remove/replace (+ basic 'test').

This is intentionally small and dependency-free (stdlib only).
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Tuple, Union


class PatchError(Exception):
    pass


def _json_pointer_unescape(seg: str) -> str:
    return seg.replace("~1", "/").replace("~0", "~")


def _split_pointer(ptr: str) -> List[str]:
    if ptr == "":
        return []
    if not ptr.startswith("/"):
        raise PatchError(f"Invalid JSON pointer (must start with '/'): {ptr!r}")
    parts = ptr.split("/")[1:]
    return [_json_pointer_unescape(p) for p in parts]


def _get_parent(doc: Any, ptr: str) -> Tuple[Any, Union[str, int]]:
    """
    Return (parent, key) where key is last segment; parent is container holding it.
    For root pointer '', raises PatchError (no parent).
    """
    parts = _split_pointer(ptr)
    if not parts:
        raise PatchError("Pointer refers to document root; no parent.")
    cur = doc
    for seg in parts[:-1]:
        if isinstance(cur, list):
            try:
                idx = int(seg)
            except ValueError:
                raise PatchError(f"Expected list index in pointer, got {seg!r}")
            if idx < 0 or idx >= len(cur):
                raise PatchError(f"Index out of bounds at segment {seg!r}")
            cur = cur[idx]
        elif isinstance(cur, dict):
            if seg not in cur:
                raise PatchError(f"Missing object key {seg!r} while traversing {ptr!r}")
            cur = cur[seg]
        else:
            raise PatchError(f"Cannot traverse into non-container at segment {seg!r}")
    last = parts[-1]
    if isinstance(cur, list):
        if last == "-":
            return cur, last
        try:
            return cur, int(last)
        except ValueError:
            raise PatchError(f"Expected list index at final segment, got {last!r}")
    elif isinstance(cur, dict):
        return cur, last
    else:
        raise PatchError(f"Pointer parent is not a container for {ptr!r}")


def _pointer_get(doc: Any, ptr: str) -> Any:
    parts = _split_pointer(ptr)
    cur = doc
    for seg in parts:
        if isinstance(cur, list):
            try:
                idx = int(seg)
            except ValueError:
                raise PatchError(f"Expected list index in pointer, got {seg!r}")
            if idx < 0 or idx >= len(cur):
                raise PatchError(f"Index out of bounds at segment {seg!r}")
            cur = cur[idx]
        elif isinstance(cur, dict):
            if seg not in cur:
                raise PatchError(f"Missing object key {seg!r} while reading {ptr!r}")
            cur = cur[seg]
        else:
            raise PatchError(f"Cannot traverse into non-container at segment {seg!r}")
    return cur


def _op_add(doc: Any, path: str, value: Any) -> None:
    parent, key = _get_parent(doc, path)
    if isinstance(parent, list):
        if key == "-":
            parent.append(value)
        else:
            idx = key
            if idx < 0 or idx > len(parent):
                raise PatchError(f"add index out of bounds: {idx}")
            parent.insert(idx, value)
    else:
        parent[key] = value


def _op_remove(doc: Any, path: str) -> None:
    parent, key = _get_parent(doc, path)
    if isinstance(parent, list):
        idx = key
        if idx < 0 or idx >= len(parent):
            raise PatchError(f"remove index out of bounds: {idx}")
        del parent[idx]
    else:
        if key not in parent:
            raise PatchError(f"remove missing key: {key!r}")
        del parent[key]


def _op_replace(doc: Any, path: str, value: Any) -> None:
    # spec: path must exist
    _ = _pointer_get(doc, path)
    _op_remove(doc, path)
    _op_add(doc, path, value)


def _op_test(doc: Any, path: str, value: Any) -> None:
    cur = _pointer_get(doc, path)
    if cur != value:
        raise PatchError(f"test failed at {path!r}: expected {value!r}, got {cur!r}")


def apply_json_patch(doc: Any, patch_ops: List[Dict[str, Any]]) -> Any:
    for op in patch_ops:
        if not isinstance(op, dict):
            raise PatchError(f"Patch op is not an object: {op!r}")
        typ = op.get("op")
        path = op.get("path")
        if not isinstance(typ, str) or not isinstance(path, str):
            raise PatchError(f"Patch op missing op/path: {op!r}")
        if typ == "add":
            _op_add(doc, path, op.get("value"))
        elif typ == "remove":
            _op_remove(doc, path)
        elif typ == "replace":
            _op_replace(doc, path, op.get("value"))
        elif typ == "test":
            _op_test(doc, path, op.get("value"))
        else:
            raise PatchError(f"Unsupported patch op: {typ!r}")
    return doc


def run_linter(x07c: str, file_path: Path) -> Tuple[int, str, str]:
    """
    Runs x07c lint and returns (exit_code, stdout, stderr).
    The linter is expected to emit x07Diag JSON on stdout.
    """
    cmd_variants = [
        [x07c, "lint", "--format", "x07diag-json", "--path", str(file_path)],
        [x07c, "lint", "--emit-json", "--path", str(file_path)],
        [x07c, "lint", "--json", "--path", str(file_path)],
        [x07c, "lint", str(file_path)],
    ]
    last = (127, "", f"x07c not found: {x07c}")
    for cmd in cmd_variants:
        try:
            p = subprocess.run(cmd, capture_output=True, text=True)
        except FileNotFoundError:
            return (127, "", f"x07c not found: {x07c}")
        # Heuristic: if subcommand/flag unsupported, try next variant.
        stderr = p.stderr.lower()
        if "unknown" in stderr or "unrecognized" in stderr or "unexpected argument" in stderr:
            last = (p.returncode, p.stdout, p.stderr)
            continue
        return (p.returncode, p.stdout, p.stderr)
    return last


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", required=True, help="Path to x07AST JSON file (e.g., src/main.x07.json).")
    ap.add_argument("--apply-patch", help="Path to JSON Patch file (RFC 6902) to apply to the x07AST.")
    ap.add_argument("--in-place", action="store_true", help="Write patched x07AST back to --file.")
    ap.add_argument("--x07c", default=os.environ.get("X07C", "x07c"), help="x07c binary (default: x07c)")
    args = ap.parse_args()

    file_path = Path(args.file)
    out: Dict[str, Any] = {
        "ok": False,
        "lint_exit_code": None,
        "diagnostics": None,
        "applied_patch": False,
        "files_modified": [],
    }

    if not file_path.exists():
        out["diagnostics"] = {
            "schema": "x07diag@0.1.0",
            "errors": [{
                "code": "X07FILE_NOT_FOUND",
                "message": f"File does not exist: {str(file_path)}",
                "path": str(file_path),
            }],
            "warnings": [],
        }
        print(json.dumps(out, ensure_ascii=False, sort_keys=True))
        return 2

    # Load JSON (x07AST).
    try:
        doc = json.loads(file_path.read_text(encoding="utf-8"))
    except Exception as e:
        out["diagnostics"] = {
            "schema": "x07diag@0.1.0",
            "errors": [{
                "code": "X07JSON_PARSE",
                "message": f"Invalid JSON (x07AST): {e}",
                "path": str(file_path),
            }],
            "warnings": [],
        }
        print(json.dumps(out, ensure_ascii=False, sort_keys=True))
        return 1

    # Apply patch if requested.
    if args.apply_patch:
        patch_path = Path(args.apply_patch)
        try:
            patch_ops = json.loads(patch_path.read_text(encoding="utf-8"))
            if not isinstance(patch_ops, list):
                raise PatchError("Patch file must be a JSON array of ops.")
            apply_json_patch(doc, patch_ops)
            out["applied_patch"] = True
            if args.in_place:
                file_path.write_text(json.dumps(doc, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
                out["files_modified"].append(str(file_path))
        except Exception as e:
            out["diagnostics"] = {
                "schema": "x07diag@0.1.0",
                "errors": [{
                    "code": "X07PATCH_APPLY_FAILED",
                    "message": str(e),
                    "path": str(patch_path),
                }],
                "warnings": [],
            }
            print(json.dumps(out, ensure_ascii=False, sort_keys=True))
            return 1

    # Run x07c linter.
    exit_code, stdout, stderr = run_linter(args.x07c, file_path)
    out["lint_exit_code"] = exit_code

    diag_obj: Any = None
    if stdout.strip():
        try:
            diag_obj = json.loads(stdout)
        except Exception:
            # Wrap non-JSON into a deterministic diagnostic.
            diag_obj = {
                "schema": "x07diag@0.1.0",
                "errors": [{
                    "code": "X07LINT_NON_JSON",
                    "message": "x07c lint did not emit JSON. See raw_stdout_b64.",
                    "path": str(file_path),
                }],
                "warnings": [],
                "raw_stdout_b64": base64.b64encode(stdout.encode("utf-8")).decode("ascii"),
            }
    else:
        diag_obj = {
            "schema": "x07diag@0.1.0",
            "errors": [{
                "code": "X07LINT_EMPTY_STDOUT",
                "message": "x07c lint emitted empty stdout; expected x07Diag JSON.",
                "path": str(file_path),
            }],
            "warnings": [],
        }

    out["diagnostics"] = diag_obj
    out["ok"] = (exit_code == 0)
    # Keep stderr out of the main structure unless you want it; agents can re-run with verbose.
    if not out["ok"] and stderr.strip():
        out["diagnostics"].setdefault("notes", [])
        out["diagnostics"]["notes"].append({"code": "X07LINT_STDERR", "message": stderr.strip()[:4000]})

    print(json.dumps(out, ensure_ascii=False, sort_keys=True))
    return 0 if out["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
````

---

## 2) Skill: x07-build-run

### Files

```
.codex/skills/x07-build-run/
  SKILL.md
  scripts/
    build_run.py
```

### `.codex/skills/x07-build-run/SKILL.md`

````md
---
name: x07-build-run
description: Compile X07 (x07AST JSON) to native via the C backend and run it, returning a machine-readable run report (stdout bytes + stderr text).
metadata:
  short-description: Build + run X07 programs
  version: 0.1.0
  kind: script-backed
---

# x07-build-run

This skill provides a **single deterministic CLI** for an autonomous agent to:
- compile X07 to a native executable using the **C backend**, and
- run the resulting program with binary stdin, capturing binary stdout.

## When to use

Use this skill when:
- you need to validate that a generated X07 program actually runs,
- you want a single JSON report for compilation + execution (good for CI and agent loops).

## Inputs

- `--entry PATH` (default: `src/main.x07.json`)
- `--module-root DIR` (default: `src`)
- `--world NAME` (default: `solve-pure`)
  - For production, you can also use `run-os` / `run-os-sandboxed` **if your toolchain provides them**.
- `--stdin PATH` (optional): binary file passed to the program via stdin
- `--out-dir DIR` (default: `target/x07-build`)
- `--cc CC` (default: `cc`)
- `--cflags "..."`
- `--timeout-ms N` (default: 5000)

## Outputs

Prints a single JSON object to stdout:

```json
{
  "compile": { "ok": true, "exit_code": 0, "c_path": "...", "exe_path": "..." },
  "run": { "ok": true, "exit_code": 0, "stdout_b64": "...", "stderr": "..." }
}
````

## Notes

* This skill intentionally does not fetch dependencies. Pair with `x07-package` for pinning/vendoring.
* If your X07 compiler already has a `build` subcommand, update the script to call it directly.

````

### `.codex/skills/x07-build-run/scripts/build_run.py`

```python
#!/usr/bin/env python3
"""
Deterministic build+run wrapper for X07 (C backend).

Assumptions (can be adjusted as your toolchain changes):
- `x07c compile ... --emit-c <path>` emits a self-contained C file.
- A system C compiler (cc/clang/gcc) is available.
- The produced executable reads stdin bytes and writes stdout bytes (solve-style).

This wrapper is intended for autonomous agents: it emits one JSON report to stdout.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any, Dict, List, Tuple


def try_compile(x07c: str, entry: Path, module_root: Path, world: str, out_c: Path) -> Tuple[bool, Dict[str, Any]]:
    cmd_variants = [
        [x07c, "compile", "--entry", str(entry), "--module-root", str(module_root), "--world", world, "--emit-c", str(out_c)],
        [x07c, "compile", "--entry", str(entry), "--module-root", str(module_root), "--world", world, "--out-c", str(out_c)],
        [x07c, "compile", str(entry), "--module-root", str(module_root), "--world", world, "--emit-c", str(out_c)],
    ]
    last: Dict[str, Any] = {"cmd": None, "exit_code": 127, "stdout": "", "stderr": f"x07c not found: {x07c}"}
    for cmd in cmd_variants:
        try:
            p = subprocess.run(cmd, capture_output=True, text=True)
        except FileNotFoundError:
            return False, last
        last = {"cmd": cmd, "exit_code": p.returncode, "stdout": p.stdout, "stderr": p.stderr}
        # If the CLI variant is wrong, try next.
        stderr_l = (p.stderr or "").lower()
        if "unknown" in stderr_l or "unrecognized" in stderr_l or "unexpected argument" in stderr_l:
            continue
        # Otherwise accept result.
        return (p.returncode == 0), last
    return False, last


def cc_compile(cc: str, c_path: Path, exe_path: Path, cflags: List[str]) -> Tuple[bool, Dict[str, Any]]:
    cmd = [cc, "-std=c11", "-O2", "-o", str(exe_path), str(c_path)] + cflags
    try:
        p = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        return False, {"cmd": cmd, "exit_code": 127, "stdout": "", "stderr": f"C compiler not found: {cc}"}
    return (p.returncode == 0), {"cmd": cmd, "exit_code": p.returncode, "stdout": p.stdout, "stderr": p.stderr}


def run_exe(exe_path: Path, stdin_bytes: bytes, timeout_ms: int) -> Tuple[bool, Dict[str, Any]]:
    t0 = time.time()
    try:
        p = subprocess.run([str(exe_path)], input=stdin_bytes, capture_output=True, timeout=timeout_ms / 1000.0)
        dt = time.time() - t0
        return (p.returncode == 0), {
            "cmd": [str(exe_path)],
            "exit_code": p.returncode,
            "time_s": dt,
            "stdout_b64": base64.b64encode(p.stdout).decode("ascii"),
            "stderr": (p.stderr or b"").decode("utf-8", errors="replace")[:4000],
        }
    except subprocess.TimeoutExpired as e:
        dt = time.time() - t0
        return False, {
            "cmd": [str(exe_path)],
            "exit_code": 124,
            "time_s": dt,
            "stdout_b64": base64.b64encode(e.stdout or b"").decode("ascii"),
            "stderr": ("TIMEOUT\n" + ((e.stderr or b"").decode("utf-8", errors="replace")))[:4000],
        }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--entry", default="src/main.x07.json")
    ap.add_argument("--module-root", default="src")
    ap.add_argument("--world", default=os.environ.get("X07_WORLD", "solve-pure"))
    ap.add_argument("--stdin", help="Optional input file passed to program stdin.")
    ap.add_argument("--out-dir", default="target/x07-build")
    ap.add_argument("--x07c", default=os.environ.get("X07C", "x07c"))
    ap.add_argument("--cc", default=os.environ.get("CC", "cc"))
    ap.add_argument("--cflags", default="", help="Extra flags, e.g. '-g -O0'.")
    ap.add_argument("--timeout-ms", type=int, default=5000)
    ap.add_argument("--write-stdout", help="Write raw stdout bytes to this path.")
    args = ap.parse_args()

    entry = Path(args.entry)
    module_root = Path(args.module_root)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    report: Dict[str, Any] = {"compile": {}, "cc": {}, "run": {}}

    if not entry.exists():
        report["compile"] = {"ok": False, "exit_code": 2, "error": f"entry not found: {str(entry)}"}
        print(json.dumps(report, sort_keys=True))
        return 2

    c_path = out_dir / "program.c"
    exe_path = out_dir / "program.exe"

    ok, comp = try_compile(args.x07c, entry, module_root, args.world, c_path)
    report["compile"] = {
        "ok": ok,
        "exit_code": comp.get("exit_code"),
        "cmd": comp.get("cmd"),
        "stdout": (comp.get("stdout") or "")[-4000:],
        "stderr": (comp.get("stderr") or "")[-4000:],
        "c_path": str(c_path) if c_path.exists() else None,
    }
    if not ok:
        print(json.dumps(report, sort_keys=True))
        return 1

    ok2, cc_rep = cc_compile(args.cc, c_path, exe_path, args.cflags.split() if args.cflags.strip() else [])
    report["cc"] = {
        "ok": ok2,
        "exit_code": cc_rep.get("exit_code"),
        "cmd": cc_rep.get("cmd"),
        "stdout": (cc_rep.get("stdout") or "")[-4000:],
        "stderr": (cc_rep.get("stderr") or "")[-4000:],
        "exe_path": str(exe_path) if exe_path.exists() else None,
    }
    if not ok2:
        print(json.dumps(report, sort_keys=True))
        return 1

    stdin_bytes = b""
    if args.stdin:
        stdin_path = Path(args.stdin)
        stdin_bytes = stdin_path.read_bytes()

    ok3, run_rep = run_exe(exe_path, stdin_bytes, args.timeout_ms)
    report["run"] = run_rep
    report["run"]["ok"] = ok3

    if args.write_stdout and "stdout_b64" in run_rep:
        out_bytes = base64.b64decode(run_rep["stdout_b64"].encode("ascii"))
        Path(args.write_stdout).write_bytes(out_bytes)

    print(json.dumps(report, sort_keys=True))
    return 0 if (ok and ok2 and ok3) else 1


if __name__ == "__main__":
    raise SystemExit(main())
````

---

## 3) Skill: x07-package

### Files

```
.codex/skills/x07-package/
  SKILL.md
  scripts/
    package.py
```

### `.codex/skills/x07-package/SKILL.md`

````md
---
name: x07-package
description: Manage X07 project manifests and lockfiles for reproducible builds (init, lock, verify). Designed for autonomous agents.
metadata:
  short-description: X07 manifest + lockfile management
  version: 0.1.0
  kind: script-backed
---

# x07-package

This skill helps an agent keep X07 projects reproducible by maintaining:

- `x07.json` — project manifest (human/agent edited)
- `x07.lock` — deterministic lockfile (machine generated)

## When to use

Use this skill when:
- creating a new X07 project,
- adding/updating dependencies,
- ensuring the repo contains a lockfile that matches the manifest.

## Manifest format (v0)

`x07.json`:

```json
{
  "manifest_version": 0,
  "name": "my-app",
  "version": "0.1.0",
  "entry": "src/main.x07.json",
  "module_root": "src",
  "world": "solve-pure",
  "deps": [
    { "id": "x07:stdlib@0.1.1", "source": "path", "path": "deps/x07/stdlib" }
  ]
}
````

## Lockfile format (v0)

`x07.lock`:

```json
{
  "lock_version": 0,
  "deps": [
    { "id": "x07:stdlib@0.1.1", "source": "path", "path": "deps/x07/stdlib", "sha256": "..." }
  ]
}
```

## Commands

* Initialize:

  * `python3 scripts/package.py init`

* Generate lock:

  * `python3 scripts/package.py lock`

* Verify lock is up to date:

  * `python3 scripts/package.py verify`

## Determinism rules

* Directory hashing is stable: lexicographic file order, hash over (relative_path + file_bytes).
* No timestamps in the lockfile.
* No network access in v0 (path deps only).

````

### `.codex/skills/x07-package/scripts/package.py`

```python
#!/usr/bin/env python3
"""
Minimal, deterministic manifest+lockfile manager for X07 projects.

v0 scope:
- path dependencies only (no network fetch).
- stable directory hashing (path + bytes, lexicographic order).
- deterministic JSON output (sorted keys, no timestamps).

This is a *starting point* for an agent-friendly packaging workflow.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import hashlib
from pathlib import Path
from typing import Any, Dict, List, Tuple


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def hash_path(path: Path) -> str:
    """
    Deterministic hash for a file or directory.
    For directories: sha256 over concatenation of (relative_path + NUL + file_bytes) for all files,
    with relative paths sorted lexicographically.
    """
    if path.is_file():
        return sha256_bytes(path.read_bytes())
    if not path.is_dir():
        raise FileNotFoundError(str(path))

    items: List[Tuple[str, bytes]] = []
    for p in sorted(path.rglob("*")):
        if p.is_file():
            rel = p.relative_to(path).as_posix()
            items.append((rel, p.read_bytes()))

    h = hashlib.sha256()
    for rel, data in items:
        h.update(rel.encode("utf-8"))
        h.update(b"\0")
        h.update(data)
        h.update(b"\n")
    return h.hexdigest()


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, obj: Any) -> None:
    path.write_text(json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def cmd_init(manifest_path: Path) -> int:
    if manifest_path.exists():
        print(f"manifest already exists: {manifest_path}", file=sys.stderr)
        return 1
    manifest = {
        "manifest_version": 0,
        "name": "my-app",
        "version": "0.1.0",
        "entry": "src/main.x07.json",
        "module_root": "src",
        "world": "solve-pure",
        "deps": [
            # Example pinned stdlib vendored into deps/x07/stdlib
            {"id": "x07:stdlib@0.1.1", "source": "path", "path": "deps/x07/stdlib"},
        ],
    }
    write_json(manifest_path, manifest)
    return 0


def cmd_lock(manifest_path: Path, lock_path: Path) -> int:
    manifest = read_json(manifest_path)
    deps = manifest.get("deps", [])
    if not isinstance(deps, list):
        raise SystemExit("manifest.deps must be a list")

    locked: List[Dict[str, Any]] = []
    for dep in deps:
        if not isinstance(dep, dict):
            raise SystemExit("each dep must be an object")
        src = dep.get("source")
        if src != "path":
            raise SystemExit(f"v0 only supports path deps; got source={src!r}")
        dep_path = Path(dep.get("path", ""))
        if not dep_path.exists():
            raise SystemExit(f"dep path missing: {dep_path}")
        locked.append({
            "id": dep.get("id"),
            "source": "path",
            "path": dep.get("path"),
            "sha256": hash_path(dep_path),
        })

    # Stable ordering by id then path.
    locked.sort(key=lambda d: (str(d.get("id")), str(d.get("path"))))

    lock = {"lock_version": 0, "deps": locked}
    write_json(lock_path, lock)
    return 0


def cmd_verify(manifest_path: Path, lock_path: Path) -> int:
    if not lock_path.exists():
        print(f"missing lockfile: {lock_path}", file=sys.stderr)
        return 1
    expected = read_json(lock_path)
    tmp_path = lock_path.with_suffix(".lock.tmp")
    cmd_lock(manifest_path, tmp_path)
    got = read_json(tmp_path)
    tmp_path.unlink(missing_ok=True)

    if expected != got:
        print("lockfile out of date (expected != recomputed)", file=sys.stderr)
        # Print a small diff hint deterministically.
        print(json.dumps({"expected": expected, "recomputed": got}, sort_keys=True)[:4000], file=sys.stderr)
        return 1
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", default=os.environ.get("X07_MANIFEST", "x07.json"))
    ap.add_argument("--lock", default=os.environ.get("X07_LOCK", "x07.lock"))
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("init")
    sub.add_parser("lock")
    sub.add_parser("verify")
    args = ap.parse_args()

    manifest_path = Path(args.manifest)
    lock_path = Path(args.lock)

    if args.cmd == "init":
        return cmd_init(manifest_path)
    if args.cmd == "lock":
        return cmd_lock(manifest_path, lock_path)
    if args.cmd == "verify":
        return cmd_verify(manifest_path, lock_path)
    raise SystemExit("unknown command")


if __name__ == "__main__":
    raise SystemExit(main())
````

---

## Why this layout matches “skills” best practices

* A skill is a **directory containing a `SKILL.md`** (with YAML frontmatter: `name`, `description`) and can include `scripts/` and other resources. ([OpenAI Developers][1])
* Codex specifically supports repo skills at `$REPO_ROOT/.codex/skills`. ([OpenAI Developers][1])

[1]: https://developers.openai.com/codex/skills/ "Agent Skills"
[2]: https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills "Equipping agents for the real world with Agent Skills \ Anthropic"

+++++
Below is a **PR-sized, production-hardening checklist** for the three standalone X07 skills you already created under `.codex/skills/…`:

* `x07-lint-repair`
* `x07-build-run`
* `x07-package`

This follows the “skills are a folder with `SKILL.md` + optional `scripts/`” model used by Codex/Claude skills.

---

## Global production contract (applies to all three skills)

These are the invariants you want in production so **agents can reliably parse + learn from tool outputs**:

### A. Output contract (hard)

1. **Stdout is always exactly one JSON object** (even on errors).

   * No extra lines, no logs, no human prose.
2. The JSON object has a top-level **envelope** that is stable across skills:

   * `schema`: `"x07.skill-report@0.1.0"` (or similar)
   * `skill`: `"x07-lint-repair"` / `"x07-build-run"` / `"x07-package"`
   * `ok`: boolean
   * `exit_code`: integer (mirrors process exit code)
   * `diagnostics`: **x07Diag JSON** (even for infra failures)
   * `artifacts`: optional outputs (paths, hashes, stdout_b64, etc.)
3. Any logs go to **stderr** only (and should be bounded/truncated).

This makes skills “tool-like” and aligns with how agents select and use skills.

### B. Exit codes (unified across skills)

Pick one table and enforce it everywhere:

* `0` — success (`ok=true`)
* `10` — invalid CLI usage (arg parsing, missing required flag)
* `20` — invalid input data (bad JSON, invalid schema)
* `30` — recoverable “user program” errors (lint errors, compile errors, lock mismatch)
* `40` — toolchain missing / incompatible version (`x07c`/`cc` not found, version mismatch)
* `50` — timeout / resource limit hit
* `70` — internal error (uncaught exception / bug)

The key is: **agents don’t guess**. They branch on `(exit_code, diagnostics.errors[*].code)`.

### C. RFC6902 JSON Patch (repair) contract

If you accept repair patches, treat them as **RFC 6902 JSON Patch** operations.
Production rules:

* Only allow: `add`, `remove`, `replace`, `test` (no `move`/`copy` unless you truly need them).
* Enforce a max op count and max patch byte size.
* Enforce a “safe pointer allowlist” for where patches may touch.

### D. Version gating

Every skill should check:

* `x07c --version` returns a compatible semver range (or an exact build hash).
* If incompatible: return `exit_code=40` with a deterministic diag code like `X07TOOL_VERSION_MISMATCH`.

This prevents agents from “thrashing” when tool output formats change.

---

## PR checklist

### PR SKILLS-01: Add a shared JSON report envelope + unified exit codes

**Goal:** every skill prints the same top-level JSON shape and uses the same exit code table.

**Changes**

* Add a tiny shared helper module:

```
.codex/skills/_shared/x07skill_report.py
```

Responsibilities:

* `emit_report(skill_name, ok, exit_code, diagnostics, artifacts)`

* guarantee: stdout is exactly JSON, sorted keys, no NaNs, bounded strings

* `try/except` wrapper that converts any exception into:

  * `exit_code=70`
  * `diagnostics.errors=[{code:"X07SKILL_INTERNAL", …}]`

* Update:

  * `.codex/skills/x07-lint-repair/scripts/lint_repair.py`
  * `.codex/skills/x07-build-run/scripts/build_run.py`
  * `.codex/skills/x07-package/scripts/package.py`

to use the shared report emitter and exit codes.

**CI gate**

* `python3 -m py_compile` on all skill scripts
* Run each skill with an intentional “error” and assert:

  * exit code matches expected
  * stdout parses as JSON object

---

### PR SKILLS-02: Define output JSON Schemas for each skill + validate in CI

**Goal:** “strict JSON output guarantees” that don’t regress.

**Changes**

* Add schemas:

```
spec/skills/skill-report.schema.json              # shared envelope
spec/skills/x07-lint-repair.report.schema.json
spec/skills/x07-build-run.report.schema.json
spec/skills/x07-package.report.schema.json
```

* Add CI script:

```
scripts/check_skills_outputs.py
```

What it does:

* executes each skill in “demo mode” (use fixture inputs)
* validates stdout JSON against schema

  * use `jsonschema` in CI, or implement a minimal validator if you insist on stdlib-only

Why: it forces you to keep outputs agent-parseable long-term.

(Using schemas/structured outputs is a common way to keep LLM-facing contracts stable. )

**CI gate**

* `python3 scripts/check_skills_outputs.py`

---

### PR SKILLS-03: Hardening `x07-lint-repair` into a true agent repair primitive

**Goal:** make repair loops **converge**, not just “apply patch and hope”.

**Changes**

1. **Require a single stable linter invocation** (stop guessing flags).

   * Standardize on:
     `x07c lint --format x07diag-json --path <file>`
   * If unsupported: exit `40` with `X07C_LINT_UNSUPPORTED`.

2. **Patch validation before apply**

   * Validate patch JSON shape as RFC6902 list.
   * Enforce:

     * max ops (e.g. 128)
     * max patch bytes (e.g. 64KB)
     * allowed ops only
     * pointer allowlist (ex: only allow changes under `/program` or `/module/items` depending on your x07AST)

3. **Schema validation after apply**

   * After patch, validate x07AST against `spec/x07ast.schema.json` (your existing contract).
   * If invalid: exit `20`, `X07PATCH_PRODUCED_INVALID_X07AST`.

4. Add `--autofix` (deterministic)

   * Implement a small set of **safe, semantics-preserving** fixes:

     * wrap multi-expression blocks in `begin` in the JSON AST representation
     * fix missing required fields with defaults (only when unambiguous)
     * canonicalize ordering of some lists (if your AST contract wants it)
   * Output: `artifacts.autofix_patch` as RFC6902 patch.
   * Must be **idempotent**: running twice yields empty patch.

5. Add `--max-iterations N`

   * When both `--apply-patch` and `--autofix` are present, allow:

     * lint → autofix → lint (repeat N times)
   * Still deterministic.

**CI gates**

* Golden tests:

  * a known-bad x07AST fixture + expected diag codes
  * ensure `--autofix` reduces errors
  * ensure idempotency: second autofix outputs empty patch
* Contract test:

  * patched x07AST validates against `x07ast.schema.json`
  * `diagnostics` validates against `x07diag.schema.json`

---

### PR SKILLS-04: Hardening `x07-build-run` (sandbox + deterministic environment)

**Goal:** prevent runaway binaries and make build/run results stable for agents.

**Changes**

1. Add resource limits (POSIX):

   * CPU time, address space, file size, open files
   * implement via `resource.setrlimit` in Python (when available)
   * if unsupported platform: emit diag `X07SKILL_RLIMIT_UNSUPPORTED` but still run

2. Add deterministic env defaults:

   * clear environment except allowlist
   * set:

     * `LANG=C`, `LC_ALL=C`, `TZ=UTC`
     * optional `SOURCE_DATE_EPOCH=0` (for reproducible builds)
   * record env in report (or record a hash of env keys/values)

3. Add stable artifact hashing:

   * `artifacts.c_sha256`
   * `artifacts.exe_sha256`
   * makes caching and debugging much easier

4. Standardize compilation CLI (stop guessing flags):

   * require one compile command shape, fail with `exit_code=40` otherwise

5. Extend report:

   * always include:

     * `compile.stderr_b64` and `cc.stderr_b64` (bounded)
     * `run.stdout_b64`
   * keep raw bytes safe (base64 only)

**CI gates**

* Build/run a tiny known-good sample module under `tests/fixtures/`:

  * ensure exit code 0
  * ensure stdout_b64 decodes properly
* Run an infinite-loop sample and ensure timeout/rlimit triggers `exit_code=50`.

---

### PR SKILLS-05: Hardening `x07-package` (fully agent-parseable, strict)

**Goal:** make dependency pinning/locking deterministic and “machine-only”.

**Changes**

1. Change `package.py` to **always output JSON** to stdout (like the other two skills).

   * no freeform stderr errors (stderr only for debug)
2. Add `spec/x07-manifest.schema.json` + `spec/x07-lock.schema.json`
3. Harden hashing:

   * define explicit exclude globs:

     * `.git/**`, `target/**`, `**/__pycache__/**`, `**/*.tmp`
   * define symlink policy:

     * either forbid symlinks or hash link target bytes deterministically (I recommend: forbid in v0)
4. Add subcommands:

   * `check` (schema validation)
   * `lock` (generate)
   * `verify` (recompute and compare)

**CI gates**

* A small fixture dep directory is hashed; verify hash stable across runs.
* `verify` fails when a file changes; error JSON includes a deterministic code.

---

### PR SKILLS-06: Wire the three skills into a production Solve/Repair loop contract

**Goal:** the agent learns from diagnostics automatically.

**Deliverable**
Add:

```
docs/agent/solve-repair-loop.md
```

with an **exact algorithm** the agent follows:

1. `x07-package verify`

   * if `exit_code=30`: run `x07-package lock`, then verify again
2. `x07-lint-repair --file <entry> --autofix --max-iterations 2`

   * if still failing:

     * read `diagnostics.errors[*]`
     * produce RFC6902 JSON Patch
     * apply: `--apply-patch … --in-place`
3. `x07-build-run --entry <entry> --module-root <src> --stdin <case>`
4. If runtime output wrong:

   * do **semantic repair** (agent changes program logic)
   * still: lint first, then build/run

**Key requirement:** the agent is only allowed to “repair” by emitting **RFC6902 JSON Patch** (not prose), so it stays tool-driven and deterministic.

Also update each SKILL.md “Workflow” section to match this loop (so skill selection is consistent).

---

### PR SKILLS-07: Add CI job “skills-contracts” (never regress)

**Goal:** skills stay production-grade and don’t drift.

**CI workflow** (example high-level steps)

1. `python3 -m py_compile` on all skill scripts
2. `python3 scripts/check_contracts.py`

   * validate x07AST / x07Diag / JSON Patch apply
3. `python3 scripts/check_skills_outputs.py`

   * run each skill on fixtures and validate report schema
4. End-to-end loop:

   * start from a known-broken x07AST fixture
   * lint → autofix → lint
   * build-run a small test input
   * assert success + stable output shape

---

## “Bite harder” reliability improvements (optional but very high ROI)

These are extra PRs that significantly improve autonomous reliability without changing language semantics:

### PR SKILLS-08: Deterministic “repair hints” in x07Diag (agent-friendly)

If your linter can emit:

* `json_pointer` to the failing node
* `expected`/`got` shape
* **suggested_patch** (small RFC6902 patch) for trivial errors

…the agent repairs more often without re-synthesizing code.

(Still: patch stays RFC6902. )

### PR SKILLS-09: Formatter skill (even if “LLM-only”)

Even in LLM-only languages, a formatter helps because:

* it canonicalizes AST layout
* it reduces diff noise
* it can normalize “equivalent” forms so repair patches are smaller

This can be a fourth skill later: `x07-format`.

---

## Why this is the right direction for 100% agentic coding

* **Skills become deterministic tools** with strict contracts, not “best effort scripts.”
* **Diagnostics become training data** for the agent’s repair policy (code → diag codes → patch templates).
* **RFC6902 patches** give you a universal, machine-checkable “edit language” for autonomous fixes.
* CI ensures you never silently regress into “sometimes prints prose” or “sometimes exits 0 but not ok”.
++++

Below are **copy/paste-ready** files for **PR SKILLS‑02**:

* 3 **JSON Schema** files (one per skill report) placed *inside each skill folder* (Codex skills explicitly allow bundling “templates and schemas” with a skill).
* a **`scripts/check_skills_outputs.py`** skeleton that:

  * runs each skill against fixture descriptors
  * parses **exactly one JSON object** from stdout
  * validates it against the skill’s schema
  * enforces basic **exit-code ↔ ok** consistency

This matches the broader “skills + schemas + CI validation” approach that Codex supports (e.g., CLI schema validation patterns).

---

## 1) `.codex/skills/x07-lint-repair/schemas/report.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:x07:skills:x07-lint-repair:report@0.1.0",
  "title": "x07-lint-repair report schema",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "ok",
    "lint_exit_code",
    "diagnostics",
    "applied_patch",
    "files_modified"
  ],
  "properties": {
    "ok": {
      "type": "boolean"
    },
    "lint_exit_code": {
      "type": [
        "integer",
        "null"
      ],
      "minimum": 0
    },
    "diagnostics": {
      "$ref": "#/$defs/x07diag"
    },
    "applied_patch": {
      "type": "boolean"
    },
    "files_modified": {
      "type": "array",
      "items": {
        "type": "string",
        "minLength": 1
      }
    }
  },
  "$defs": {
    "diag_item": {
      "type": "object",
      "required": [
        "code",
        "message"
      ],
      "properties": {
        "code": {
          "type": "string",
          "minLength": 1
        },
        "message": {
          "type": "string"
        },
        "path": {
          "type": "string"
        },
        "span": {
          "type": "object"
        }
      },
      "additionalProperties": true
    },
    "x07diag": {
      "type": "object",
      "required": [
        "schema",
        "errors",
        "warnings"
      ],
      "properties": {
        "schema": {
          "type": "string",
          "minLength": 1
        },
        "errors": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/diag_item"
          }
        },
        "warnings": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/diag_item"
          }
        },
        "notes": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/diag_item"
          }
        },
        "raw_stdout_b64": {
          "type": "string"
        }
      },
      "additionalProperties": true
    }
  }
}
```

---

## 2) `.codex/skills/x07-build-run/schemas/report.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:x07:skills:x07-build-run:report@0.1.0",
  "title": "x07-build-run report schema",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "compile",
    "cc",
    "run"
  ],
  "properties": {
    "compile": {
      "$ref": "#/$defs/compile"
    },
    "cc": {
      "$ref": "#/$defs/cc_or_empty"
    },
    "run": {
      "$ref": "#/$defs/run_or_empty"
    }
  },
  "$defs": {
    "empty_object": {
      "type": "object",
      "maxProperties": 0
    },
    "compile_full": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "ok",
        "exit_code",
        "cmd",
        "stdout",
        "stderr",
        "c_path"
      ],
      "properties": {
        "ok": {
          "type": "boolean"
        },
        "exit_code": {
          "type": "integer",
          "minimum": 0
        },
        "cmd": {
          "type": [
            "array",
            "null"
          ],
          "items": {
            "type": "string",
            "minLength": 1
          }
        },
        "stdout": {
          "type": "string"
        },
        "stderr": {
          "type": "string"
        },
        "c_path": {
          "type": [
            "string",
            "null"
          ]
        }
      }
    },
    "compile_missing_entry": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "ok",
        "exit_code",
        "error"
      ],
      "properties": {
        "ok": {
          "const": false
        },
        "exit_code": {
          "type": "integer",
          "minimum": 0
        },
        "error": {
          "type": "string",
          "minLength": 1
        }
      }
    },
    "compile": {
      "anyOf": [
        {
          "$ref": "#/$defs/compile_full"
        },
        {
          "$ref": "#/$defs/compile_missing_entry"
        }
      ]
    },
    "cc_report": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "ok",
        "exit_code",
        "cmd",
        "stdout",
        "stderr",
        "exe_path"
      ],
      "properties": {
        "ok": {
          "type": "boolean"
        },
        "exit_code": {
          "type": "integer",
          "minimum": 0
        },
        "cmd": {
          "type": [
            "array",
            "null"
          ],
          "items": {
            "type": "string",
            "minLength": 1
          }
        },
        "stdout": {
          "type": "string"
        },
        "stderr": {
          "type": "string"
        },
        "exe_path": {
          "type": [
            "string",
            "null"
          ]
        }
      }
    },
    "cc_or_empty": {
      "anyOf": [
        {
          "$ref": "#/$defs/empty_object"
        },
        {
          "$ref": "#/$defs/cc_report"
        }
      ]
    },
    "run_report": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "ok",
        "cmd",
        "exit_code",
        "time_s",
        "stdout_b64",
        "stderr"
      ],
      "properties": {
        "ok": {
          "type": "boolean"
        },
        "cmd": {
          "type": [
            "array",
            "null"
          ],
          "items": {
            "type": "string",
            "minLength": 1
          }
        },
        "exit_code": {
          "type": "integer",
          "minimum": 0
        },
        "time_s": {
          "type": "number",
          "minimum": 0
        },
        "stdout_b64": {
          "type": "string"
        },
        "stderr": {
          "type": "string"
        }
      }
    },
    "run_or_empty": {
      "anyOf": [
        {
          "$ref": "#/$defs/empty_object"
        },
        {
          "$ref": "#/$defs/run_report"
        }
      ]
    }
  }
}
```

---

## 3) `.codex/skills/x07-package/schemas/report.schema.json`

This schema assumes **SKILLS‑02 also makes `x07-package` always emit a single JSON report to stdout** (even on errors), consistent with the other skills.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:x07:skills:x07-package:report@0.1.0",
  "title": "x07-package report schema",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "ok",
    "command",
    "exit_code",
    "manifest_path",
    "lock_path"
  ],
  "properties": {
    "ok": {
      "type": "boolean"
    },
    "command": {
      "type": "string",
      "enum": [
        "init",
        "lock",
        "verify"
      ]
    },
    "exit_code": {
      "type": "integer",
      "minimum": 0
    },
    "manifest_path": {
      "type": "string",
      "minLength": 1
    },
    "lock_path": {
      "type": "string",
      "minLength": 1
    },
    "manifest": {
      "type": "object"
    },
    "lock": {
      "$ref": "#/$defs/lockfile"
    },
    "written_files": {
      "type": "array",
      "items": {
        "type": "string",
        "minLength": 1
      }
    },
    "diagnostics": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/diag_item"
      }
    },
    "stderr_tail": {
      "type": "string"
    }
  },
  "$defs": {
    "diag_item": {
      "type": "object",
      "required": [
        "code",
        "message"
      ],
      "properties": {
        "code": {
          "type": "string",
          "minLength": 1
        },
        "message": {
          "type": "string"
        },
        "path": {
          "type": "string"
        }
      },
      "additionalProperties": true
    },
    "lockfile": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "lock_version",
        "deps"
      ],
      "properties": {
        "lock_version": {
          "type": "integer",
          "minimum": 0
        },
        "deps": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/locked_dep"
          }
        }
      }
    },
    "locked_dep": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "id",
        "source",
        "path",
        "sha256"
      ],
      "properties": {
        "id": {
          "type": "string",
          "minLength": 1
        },
        "source": {
          "type": "string",
          "enum": [
            "path"
          ]
        },
        "path": {
          "type": "string",
          "minLength": 1
        },
        "sha256": {
          "type": "string",
          "pattern": "^[0-9a-f]{64}$"
        }
      }
    }
  }
}
```

---

## 4) `scripts/check_skills_outputs.py`

This script expects fixtures as **JSON fixture descriptors** at:

* `.codex/skills/<skill>/fixtures/*.fixture.json`

Each fixture file should look like:

```json
{
  "name": "example",
  "cwd": ".codex/skills/x07-build-run",
  "cmd": ["python3", "scripts/build_run.py", "--entry", "fixtures/src/main.x07.json"],
  "env": { "X07C": "x07c" },
  "timeout_s": 30,
  "expect_ok": false,
  "expect_exit_code": 1
}
```

Now the script:

```python
#!/usr/bin/env python3
"""
CI hook: run Codex skills against checked-in fixtures and validate their JSON outputs.

Goals:
- Deterministic: no network, stable parsing, bounded output.
- Strict: require EXACTLY one JSON object on stdout.
- Validate against JSON Schemas committed with each skill.
- Enforce exit-code ↔ ok consistency (basic sanity).

Fixture convention:
  .codex/skills/<skill>/fixtures/*.fixture.json

Schema convention:
  .codex/skills/<skill>/schemas/report.schema.json
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


def strict_parse_one_json_object(stdout: str) -> Any:
    """
    Parse exactly one JSON value and ensure there is no trailing non-whitespace.
    This catches 'JSON + extra logs' deterministically.
    """
    s = stdout.strip()
    if not s:
        raise ValueError("stdout is empty; expected one JSON object")
    dec = json.JSONDecoder()
    obj, idx = dec.raw_decode(s)
    rest = s[idx:].strip()
    if rest:
        raise ValueError(f"stdout contains trailing non-JSON content (starts with {rest[:80]!r})")
    return obj


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def validate_with_jsonschema(schema_obj: Any, instance: Any) -> List[str]:
    """
    Returns a list of human-readable validation errors (empty list => ok).
    Uses jsonschema if available; otherwise hard-fails so CI is explicit.
    """
    try:
        import jsonschema  # type: ignore
        from jsonschema.validators import validator_for  # type: ignore
    except Exception as e:
        return [f"jsonschema not available: {e}. Add it to CI (pip install jsonschema)."]

    Validator = validator_for(schema_obj)
    Validator.check_schema(schema_obj)
    v = Validator(schema_obj)

    errors: List[str] = []
    for err in sorted(v.iter_errors(instance), key=lambda e: list(e.path)):
        path = "/".join(str(p) for p in err.path)
        loc = f"@{path}" if path else "@<root>"
        errors.append(f"{loc}: {err.message}")
    return errors


@dataclass(frozen=True)
class Fixture:
    name: str
    cwd: Path
    cmd: List[str]
    env: Dict[str, str]
    timeout_s: int
    expect_ok: Optional[bool] = None
    expect_exit_code: Optional[int] = None


def load_fixture(path: Path, repo_root: Path) -> Fixture:
    raw = load_json(path)
    if not isinstance(raw, dict):
        raise ValueError(f"fixture must be an object: {path}")

    def req_str(k: str) -> str:
        v = raw.get(k)
        if not isinstance(v, str) or not v:
            raise ValueError(f"{path}: {k} must be a non-empty string")
        return v

    name = req_str("name")
    cwd = (repo_root / req_str("cwd")).resolve()
    cmd = raw.get("cmd")
    if not isinstance(cmd, list) or not all(isinstance(x, str) and x for x in cmd):
        raise ValueError(f"{path}: cmd must be a non-empty list of strings")

    env_raw = raw.get("env", {})
    if not isinstance(env_raw, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in env_raw.items()):
        raise ValueError(f"{path}: env must be a map of string->string")
    env = {k: v for k, v in env_raw.items()}

    timeout_s = int(raw.get("timeout_s", 30))
    expect_ok = raw.get("expect_ok", None)
    if expect_ok is not None and not isinstance(expect_ok, bool):
        raise ValueError(f"{path}: expect_ok must be boolean if present")

    expect_exit_code = raw.get("expect_exit_code", None)
    if expect_exit_code is not None and not isinstance(expect_exit_code, int):
        raise ValueError(f"{path}: expect_exit_code must be int if present")

    return Fixture(
        name=name,
        cwd=cwd,
        cmd=[str(x) for x in cmd],
        env=env,
        timeout_s=timeout_s,
        expect_ok=expect_ok,
        expect_exit_code=expect_exit_code,
    )


def run_fixture(fx: Fixture) -> Tuple[int, str, str]:
    env = os.environ.copy()
    env.update(fx.env)
    p = subprocess.run(
        fx.cmd,
        cwd=str(fx.cwd),
        env=env,
        capture_output=True,
        text=True,
        timeout=fx.timeout_s,
    )
    return p.returncode, p.stdout, p.stderr


def discover_fixtures(skill_dir: Path) -> List[Path]:
    fx_dir = skill_dir / "fixtures"
    if not fx_dir.is_dir():
        return []
    return sorted(fx_dir.glob("*.fixture.json"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo-root", default=".", help="Repo root (default: .)")
    ap.add_argument("--allow-empty", action="store_true", help="If set, skip skills with no fixtures.")
    args = ap.parse_args()

    repo_root = Path(args.repo_root).resolve()
    skills_root = repo_root / ".codex" / "skills"
    if not skills_root.is_dir():
        print(f"missing skills root: {skills_root}", file=sys.stderr)
        return 2

    skill_names = [
        "x07-lint-repair",
        "x07-build-run",
        "x07-package",
    ]

    failures: List[str] = []

    for skill in skill_names:
        skill_dir = skills_root / skill
        schema_path = skill_dir / "schemas" / "report.schema.json"
        if not schema_path.exists():
            failures.append(f"{skill}: missing schema: {schema_path}")
            continue

        schema_obj = load_json(schema_path)

        fx_paths = discover_fixtures(skill_dir)
        if not fx_paths:
            msg = f"{skill}: no fixtures found under {skill_dir/'fixtures'}"
            if args.allow_empty:
                print(f"[SKIP] {msg}")
                continue
            failures.append(msg)
            continue

        for fx_path in fx_paths:
            try:
                fx = load_fixture(fx_path, repo_root)
                rc, out, err = run_fixture(fx)

                # Parse stdout as a single JSON object
                try:
                    obj = strict_parse_one_json_object(out)
                except Exception as e:
                    failures.append(f"{skill}/{fx.name}: stdout not strict JSON object: {e}. stderr_tail={err[:400]!r}")
                    continue

                # Validate schema
                schema_errors = validate_with_jsonschema(schema_obj, obj)
                if schema_errors:
                    failures.append(f"{skill}/{fx.name}: schema violations:\n  - " + "\n  - ".join(schema_errors))
                    continue

                # Minimal cross-check: ok ↔ exit_code
                ok = obj.get("ok", None)
                if isinstance(ok, bool):
                    if ok and rc != 0:
                        failures.append(f"{skill}/{fx.name}: ok=true but exit_code={rc}")
                    if (not ok) and rc == 0:
                        failures.append(f"{skill}/{fx.name}: ok=false but exit_code=0")

                # Fixture expectations (optional)
                if fx.expect_exit_code is not None and rc != fx.expect_exit_code:
                    failures.append(f"{skill}/{fx.name}: expected exit_code={fx.expect_exit_code}, got {rc}")
                if fx.expect_ok is not None and isinstance(ok, bool) and ok != fx.expect_ok:
                    failures.append(f"{skill}/{fx.name}: expected ok={fx.expect_ok}, got {ok}")

                print(f"[OK] {skill}/{fx.name}")

            except Exception as e:
                failures.append(f"{skill}: fixture {fx_path.name} failed to run/validate: {e}")

    if failures:
        print("\nSKILLS OUTPUT CHECK FAILED:\n", file=sys.stderr)
        for f in failures:
            print(f"- {f}", file=sys.stderr)
        return 1

    print("\nSKILLS OUTPUT CHECK PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

---

## Notes for SKILLS‑02 wiring (quick, practical)

* Put schemas under each skill folder because Codex skills explicitly support shipping **schemas/templates** alongside scripts/docs.
* In CI, add a job step:

```bash
python3 -m pip install --upgrade jsonschema
python3 scripts/check_skills_outputs.py --repo-root . 
```

* Longer-term: if you later want the **Solve/Repair model** to emit guaranteed-conforming JSON too, you can use JSON Schema–driven structured outputs and validate the same way.
