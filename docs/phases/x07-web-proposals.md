# X07 web proposals

Status: not implemented (design notes).

## Should we force agents into specific design choices?

You have three levers, from “hard” to “soft”. For a 100% agentic language/ecosystem, you usually want **hard constraints only where they eliminate whole classes of failure**, and **soft constraints everywhere else**.

### Lever 1: Hard constraints (language/toolchain enforced)

Use these when *ambiguity creates frequent agent failures* or *safety/perf becomes unreviewable*.

Examples that are worth forcing:

* **Capability boundaries** (pure vs OS vs sandboxed OS). This prevents “accidental nondeterminism” and makes behavior reviewable.
* **Single canonical encodings** (X7HT/X7SL/DataModel). Agents shouldn’t invent ad‑hoc encodings.
* **Error model** (Result/Option shapes + numeric spaces). Agents shouldn’t guess how to propagate errors.
* **Resource budgets** (mem/sched stats, leak gates, hard caps). Agents will otherwise “fix by copying” and blow budgets.

This matches what agent tooling guidance tends to recommend: provide clear conventions and guardrails so agents can modify many files while staying consistent. ([OpenAI Developers][1])

### Lever 2: Soft constraints (linters, “architecture rules”, repo contracts)

Use these when multiple designs are valid, but you want agents to converge to one “house style”.

Examples:

* “Functional core must not import `std.os.*`”
* “`std.net.*` adapters only in `run-os*` world targets”
* “All public APIs must have: encoding doc link + smoke test + deterministic fixture”
* “No global mutable state outside module-private state machines”

This is the sweet spot for “agent autonomy without chaos”.

### Lever 3: Conventions + templates (skills and scaffolds)

Use these when you don’t want to restrict innovation, but you want **agents to start from known-good shapes**.

Examples:

* “Create a new service/module via skeleton generator”
* “Use `std.cli` for argument parsing”
* “Use `std.io` for streaming + buffering”
* “Use adapter interfaces so the same program can run fixture-world and OS-world with only bindings swapped”

Claude Code best-practices specifically emphasizes repo-local convention files (like `CLAUDE.md`) and clear testing instructions—this is basically “conventions + templates + gates” as a first-class control system. ([Anthropic][2])

---

## What was valuable for humans but changes for agents?

### Less critical for agents

These existed mainly because humans are slow at search/recall and make “local edits”:

* Over-engineered abstraction layers “just in case”
* Heavy ceremony patterns (excessively deep inheritance trees, “design pattern for everything”)
* Micro-optimizing for “developer typing speed” (agents don’t type; they generate)

### Still valuable for agents

Even if agents write all code, they still have:

* **Context-window limits** (they can’t keep the entire system in working memory at once)
* **Repair loops** (they need deterministic feedback to converge)
* **Regression risks** (they can unintentionally break unrelated modules)

So these remain valuable:

* Clear module boundaries
* Stable interfaces/contracts
* Tests as “fitness functions”
* Deterministic build + deterministic diagnostics
* Standardized error semantics and encodings

OpenAI’s guidance for “AI-native engineering” explicitly frames agents as end-to-end implementers that still need conventions, tests, and build checks as a finish line. ([OpenAI Developers][1])

---

## Out-of-box agent-first software design principles

Here are “agent-era” principles that I’d treat as first-class, even if they feel unusual to humans:

### 1) Contracts over patterns

Replace “pattern vocabulary” with **machine-checkable contracts**:

* Interface contracts: input/output encoding specs
* Capability contracts: what world adapters are permitted
* Budget contracts: max allocations/memcpy/reallocs, max scheduler work, etc.
* Deterministic replay contracts: same inputs ⇒ same outputs/metrics

Patterns become *derived artifacts*, not the primary thing.

### 2) Architecture as data (a manifest agents must update)

Instead of relying on humans remembering architecture rules, make the repo contain a canonical artifact like:

* `arch/graph.json` (modules, dependencies, worlds)
* `arch/budgets.json` (per-module/per-endpoint budgets)
* `arch/contracts/` (encoding + error specs by module)

The linter enforces the manifest matches reality.

This is the agent-era equivalent of “type checking” but for architecture.

### 3) “Functional core, imperative shell” as the default shape

This is unusually aligned with X07’s world split.

* **Functional core**: pure transformations, parsing, routing, serialization
* **Imperative shell**: actual OS I/O, process spawning, network, filesystem

Besides being clean, it makes deterministic testing easier because the core is deterministic by construction, and only the shell needs fixture/policy scaffolding. ([blog.ploeh.dk][3])

### 4) Ports & adapters for everything that touches the world

Hexagonal / ports-and-adapters architecture is basically a formalization of the “core + adapters” idea. It’s valuable because you can swap:

* fixture adapters (deterministic) vs
* OS adapters (real world)

…without rewriting core logic. ([Wikipedia][4])

### 5) Make “repairability” a non-negotiable requirement

For 100% autonomous coding, the system must *teach the agent how to fix itself*:

* deterministic diagnostics
* stable diagnostic codes
* structured patches (AST/JSON Patch) rather than free-form edits
* a linter that suggests canonical fixes (not just errors)

This is exactly why “finish line checks” (tests, linters) matter: agents converge when the loop is crisp and deterministic. ([OpenAI Developers][1])

### 6) “Code is a cache; specs are the source of truth”

Agents can regenerate code; therefore:

* The most important artifact is the **spec** (encodings, invariants, API contract, policy)
* Code can be re-derived from spec + templates
* Humans review specs + diffs, not hand-written code style

This is a big philosophical shift: you stop treating source code as sacred and treat it as *compiled output of higher-level intent*.

### 7) Budget-aware design, everywhere

Because agents overuse copying and allocate freely unless punished:

* Put budgets into CI
* Put budgets into diagnostics
* Put budgets into codegen hints (e.g., “prefer views”, “with_capacity”)

You already do this with mem/sched stats. The key is: **make budgets part of the design contract**, not a late-stage optimization.

---

## So… should you guide the agent, or leave it “up to the LLM”?

A practical stance:

### Force these (hard):

* World/capability boundaries
* Canonical encodings + error spaces
* Deterministic build + deterministic diagnostics
* Budget instrumentation + CI gates

### Strongly guide these (soft, linted):

* Core is pure; adapters are effectful
* Stable module boundaries + import rules
* “One canonical way” APIs for common tasks

### Leave these to the agent (with tests):

* Internal decomposition within modules
* Local algorithm choices
* Micro-architecture inside a subsystem (as long as contracts hold)

This preserves innovation while preventing the “agent made a clever architecture that no longer fits in any model’s context window”.

---

## Example: agent builds a web framework in X07

If I were designing the *agent workflow* and *default architecture*, I’d use a “core + adapters” template:

### Layer 0: Protocol + core types (pure)

* HTTP parsing/serialization (pure)
* routing (pure)
* middleware chain (pure)
* request/response models (DataModel-first)
* error mapping (Result codes + canonical error body)

**No OS imports allowed.** This is unit-testable in solve-pure (or a deterministic run world).

### Layer 1: Server runtime (adapter)

* socket accept loop
* TLS
* concurrency model (your async scheduler can structure the logic; OS threads/processes can parallelize in run-os)
* streaming I/O via `std.io`

**This layer is allowed only in run-os/run-os-sandboxed.**

### Layer 2: Application code (business logic)

* user handlers, templates, DB calls, caching
* should depend on core abstractions, not raw OS calls

### What “patterns” does the agent follow?

Not “Gang of Four patterns” by default. Instead:

* “Pure core, effectful adapters”
* “Ports/adapters interface boundary”
* “Every public function returns Result + canonical error codes”
* “All external boundaries have fixture replays”

### What do you give the agent upfront?

* A module skeleton generator (“new web service” scaffold)
* A contract checklist the agent must satisfy
* A suite of deterministic fixture tests (“HTTP request transcripts”)
* Budget gates for common anti-patterns (copying bodies repeatedly, non-streaming parsing)

This is how you avoid letting the LLM invent an architecture that’s clever but brittle.

---

## A few concrete “novel” agent-era rules you can adopt

If you want opinionated rules that actually help autonomy, these are unusually high ROI:

1. **No implicit global state**: global state must be behind an explicit handle passed around (agent-friendly traceability).

2. **Everything that crosses a module boundary has a pinned encoding doc**.

3. **Every module has a “smoke harness”** that can run in fixture mode.

4. **Architectural lints are first-class** (imports/capabilities/budgets).

5. **“Refactor is expected”**: you optimize for easy refactors, not perfect upfront design. Agents are good at wide edits; make that safe.

---

## Bottom line recommendation

If X07 is meant for 100% autonomous agents, the best strategy is:

* **Hard-enforce a small set of semantics that prevent catastrophic drift** (capabilities, encodings, error spaces, budgets).
* **Soft-enforce architecture via deterministic lint rules and templates** (functional core + adapters; ports/adapters boundaries).
* **Let the agent innovate inside the boundaries**, but require contracts + smoke tests so you can safely accept wide diffs.

If you want, I can translate the above into a concrete “agent-facing architecture contract” template you’d check into repos (like a `AGENT_ARCH.md` + `arch/graph.json` + CI checks) specifically for an X07 web framework project, so agents have rails without being micromanaged.

[1]: https://developers.openai.com/codex/guides/build-ai-native-engineering-team/ "Building an AI-Native Engineering Team"
[2]: https://www.anthropic.com/engineering/claude-code-best-practices "Claude Code Best Practices \ Anthropic"
[3]: https://blog.ploeh.dk/2022/05/23/waiting-to-never-happen/ "Waiting to never happen"
[4]: https://en.wikipedia.org/wiki/Hexagonal_architecture_%28software%29 "Hexagonal architecture (software) - Wikipedia"
++++

Below is a **concrete, agent-facing architecture contract template** you can drop into any X07 “application/framework” repo (e.g., a new web framework project). It’s designed around:

* **Functional core / imperative shell** separation (testable core, thin side-effect shell). ([Destroy All Software][1])
* **Ports & adapters (hexagonal)** so you can swap fixture adapters ↔ OS adapters without rewriting core logic. ([Alistair Cockburn][2])
* **Agent-friendly “instruction discovery”** via `AGENTS.md` and deterministic gates. ([OpenAI Developers][3])

---

# Repo template: “Agent-Facing Architecture Contract”

## 0) Folder layout

```text
.
├── AGENTS.md
├── docs/
│   ├── architecture/
│   │   ├── contract-v1.md
│   │   ├── public-apis.md
│   │   └── encodings/
│   │       ├── http-req-v1.md
│   │       ├── http-resp-v1.md
│   │       └── errors.md
│   └── runbooks/
│       ├── release.md
│       └── debugging.md
├── arch/
│   ├── graph.json
│   ├── rules.json
│   ├── budgets.json
│   └── contracts.json
├── schemas/
│   ├── arch.graph.schema.json
│   ├── arch.rules.schema.json
│   ├── arch.budgets.schema.json
│   └── arch.contracts.schema.json
├── scripts/
│   ├── check_arch_contract.py
│   └── ci/
│       └── check_all.sh
├── packages/
│   ├── <your-workspace-packages>/
│   └── ... (x07AST source-only modules)
└── tests/
    ├── fixtures/
    │   ├── http/
    │   ├── fs/
    │   └── db/
    └── smoke/
        ├── fixture-world/
        └── run-os/
```

The important idea: **architecture is data** (`arch/*.json`), and CI enforces it.

---

# 1) `AGENTS.md` (agent-facing “entry point”)

Codex (and other coding agents) typically look for repo-local instruction files like `AGENTS.md`. ([OpenAI Developers][3])
This file should be **short**, **directive**, and **operational**.

```md
# AGENTS.md — X07 Project Contract (read first)

You are an autonomous coding agent. Your job is to implement features while preserving the architecture contract and all deterministic gates.

## Non‑negotiable invariants

1) Do not introduce new I/O side effects into core modules.
2) All side effects MUST go through defined ports (arch/graph.json) and be implemented via adapters.
3) Any new public API MUST have:
   - a pinned bytes encoding doc under docs/architecture/encodings/
   - a deterministic smoke test under tests/smoke/
   - an entry in arch/contracts.json
4) Keep “single canonical way”: if adding a capability, add ONE canonical API surface, not aliases.

## Where to read the real rules
- docs/architecture/contract-v1.md (normative)
- arch/graph.json, arch/rules.json, arch/budgets.json, arch/contracts.json (machine enforced)

## Required pre-commit checks
Run:
  ./scripts/ci/check_all.sh

If anything fails:
- Fix the failure.
- Do not weaken gates or remove tests to “make it pass”.
```

---

# 2) `docs/architecture/contract-v1.md` (normative contract)

This is the “constitution”. It should define the architectural choices you *do* want to force.

Why these choices work well for agent-built systems:

* **Functional core / imperative shell** isolates side effects and makes testing straightforward. ([Destroy All Software][1])
* **Ports & adapters** lets you swap real dependencies for fixtures/mocks without contaminating business logic. ([Alistair Cockburn][2])

```md
# Architecture Contract v1 (Agent-Enforced)

## Purpose
This repo must remain maintainable and safe under fully autonomous agent changes.
We optimize for:
- deterministic builds and diagnostics
- deterministic fixture replay
- contract-based module boundaries
- budget-aware performance

## Architectural shape (forced)
We use:
1) Functional Core / Imperative Shell:
   - Core modules: compute/parse/route/transform; no OS access.
   - Shell/adapters: OS I/O, networking, filesystem, DB, process spawning.

2) Ports & Adapters:
   - Ports are stable interfaces defined in packages/*.
   - Adapters implement ports for:
     - deterministic fixture worlds (tests)
     - run-os / run-os-sandboxed (production)

## Module kinds
Each module in arch/graph.json is labeled with kind:
- core: pure logic, may import ports, must not import adapters or std.os.*
- port: interface types + request/response encodings, no OS
- adapter-fixture: deterministic replay adapters (tests)
- adapter-os: OS adapters (production only)
- app: binaries/entrypoints

## Capability rules
- core MUST NOT call std.os.*, ext.os.*, or any world adapter API.
- adapters MUST be the only modules importing OS bindings.
- apps wire core + adapters.

## Public API rules
Any exported function is “public” unless explicitly internal.
For every public function:
- there is a pinned bytes encoding doc (docs/architecture/encodings/*.md)
- there is a smoke test that asserts bytes outputs exactly
- there is an entry in arch/contracts.json

## Budget rules
Budget keys are defined in arch/budgets.json and enforced by scripts/check_arch_contract.py:
- max_memcpy_bytes
- max_realloc_calls
- max_peak_live_bytes
- (optional) max_sched_cost / max_spawned_procs / etc

Budgets are “design constraints”, not optional optimizations.
```

---

# 3) `arch/graph.json` (the module dependency truth)

This is what prevents agent “architectural drift” as codebase grows.

Example graph for a web framework:

```json
{
  "schema_version": "arch.graph@0.1.0",
  "modules": [
    {
      "id": "core.http",
      "kind": "core",
      "worlds_allowed": ["fixture", "run-os", "run-os-sandboxed"],
      "imports": ["port.http", "port.time"],
      "exports": ["core.http.parse_req_v1", "core.http.write_resp_v1"]
    },
    {
      "id": "core.router",
      "kind": "core",
      "worlds_allowed": ["fixture", "run-os", "run-os-sandboxed"],
      "imports": ["port.http"],
      "exports": ["core.router.route_v1"]
    },
    {
      "id": "port.http",
      "kind": "port",
      "worlds_allowed": ["fixture", "run-os", "run-os-sandboxed"],
      "imports": [],
      "exports": ["port.http.ReqV1", "port.http.RespV1"]
    },
    {
      "id": "adapter.fixture.http",
      "kind": "adapter-fixture",
      "worlds_allowed": ["fixture"],
      "imports": ["port.http"],
      "exports": ["adapter.fixture.http.serve_v1"]
    },
    {
      "id": "adapter.os.http",
      "kind": "adapter-os",
      "worlds_allowed": ["run-os", "run-os-sandboxed"],
      "imports": ["port.http"],
      "exports": ["adapter.os.http.serve_v1"]
    },
    {
      "id": "app.server",
      "kind": "app",
      "worlds_allowed": ["fixture", "run-os", "run-os-sandboxed"],
      "imports": ["core.http", "core.router", "adapter.fixture.http", "adapter.os.http"],
      "exports": ["app.server.main"]
    }
  ]
}
```

---

# 4) `arch/rules.json` (import/capability constraints)

```json
{
  "schema_version": "arch.rules@0.1.0",
  "forbidden_imports": [
    { "from_kind": "core", "to_kind": "adapter-fixture" },
    { "from_kind": "core", "to_kind": "adapter-os" }
  ],
  "forbidden_module_prefixes": [
    { "from_kind": "core", "prefix": "std.os." },
    { "from_kind": "core", "prefix": "ext.os." }
  ],
  "allowed_edges": [
    { "from_kind": "core", "to_kind": "port" },
    { "from_kind": "adapter-fixture", "to_kind": "port" },
    { "from_kind": "adapter-os", "to_kind": "port" },
    { "from_kind": "app", "to_kind": "core" },
    { "from_kind": "app", "to_kind": "adapter-fixture" },
    { "from_kind": "app", "to_kind": "adapter-os" }
  ]
}
```

---

# 5) `arch/contracts.json` (public API → encoding + tests)

```json
{
  "schema_version": "arch.contracts@0.1.0",
  "public_apis": [
    {
      "symbol": "core.http.parse_req_v1",
      "encoding_doc": "docs/architecture/encodings/http-req-v1.md",
      "errors_doc": "docs/architecture/encodings/errors.md",
      "smoke_tests": [
        "tests/smoke/fixture-world/http_parse_smoke.json"
      ]
    },
    {
      "symbol": "core.http.write_resp_v1",
      "encoding_doc": "docs/architecture/encodings/http-resp-v1.md",
      "errors_doc": "docs/architecture/encodings/errors.md",
      "smoke_tests": [
        "tests/smoke/fixture-world/http_write_smoke.json"
      ]
    }
  ]
}
```

---

# 6) `arch/budgets.json` (performance/resource budgets)

Budgets make agent outputs predictable and prevent “copy everything until it passes”.

```json
{
  "schema_version": "arch.budgets@0.1.0",
  "defaults": {
    "max_memcpy_bytes_factor": 2.0,
    "max_realloc_calls": 5,
    "max_peak_live_bytes_factor": 4.0
  },
  "overrides": [
    {
      "module": "core.http",
      "max_memcpy_bytes_factor": 1.2,
      "max_realloc_calls": 1,
      "max_peak_live_bytes_factor": 2.0
    }
  ]
}
```

Interpretation example:

* `max_memcpy_bytes_factor = 1.2` means memcpy_bytes must be ≤ 1.2× output_size + O(1).

---

# 7) Minimal schema drafts (so it’s machine-checkable)

### `schemas/arch.graph.schema.json` (minimal)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "arch.graph.schema.json",
  "type": "object",
  "required": ["schema_version", "modules"],
  "properties": {
    "schema_version": { "type": "string" },
    "modules": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "kind", "worlds_allowed", "imports", "exports"],
        "properties": {
          "id": { "type": "string", "pattern": "^[a-z0-9_.-]+$" },
          "kind": {
            "type": "string",
            "enum": ["core", "port", "adapter-fixture", "adapter-os", "app"]
          },
          "worlds_allowed": {
            "type": "array",
            "items": { "type": "string", "enum": ["fixture", "run-os", "run-os-sandboxed"] }
          },
          "imports": { "type": "array", "items": { "type": "string" } },
          "exports": { "type": "array", "items": { "type": "string" } }
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
```

(Repeat similarly minimal schemas for rules/budgets/contracts; keep them strict.)

---

# 8) `scripts/check_arch_contract.py` (enforcement skeleton)

This is the “contract judge”. It should:

* validate JSONs against schemas
* enforce the import/capability rules
* enforce that every public API has docs + smoke tests
* optionally run smoke tests and parse mem/sched stats

Skeleton:

```python
#!/usr/bin/env python3
import json, os, sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

def load_json(p: Path):
    return json.loads(p.read_text(encoding="utf-8"))

def fail(msg: str):
    print(f"ARCH_CONTRACT_FAIL: {msg}")
    sys.exit(2)

def main():
    graph = load_json(REPO / "arch/graph.json")
    rules = load_json(REPO / "arch/rules.json")
    contracts = load_json(REPO / "arch/contracts.json")
    budgets = load_json(REPO / "arch/budgets.json")

    mods = {m["id"]: m for m in graph["modules"]}

    # 1) Basic referential integrity
    for mid, m in mods.items():
        for dep in m["imports"]:
            if dep not in mods:
                fail(f"{mid} imports unknown module {dep}")

    # 2) Enforce forbidden import edges by kind
    for m in mods.values():
        for dep in m["imports"]:
            depm = mods[dep]
            for rule in rules.get("forbidden_imports", []):
                if m["kind"] == rule["from_kind"] and depm["kind"] == rule["to_kind"]:
                    fail(f"forbidden edge: {m['id']} ({m['kind']}) -> {dep} ({depm['kind']})")

    # 3) Enforce forbidden module prefixes (for core)
    for m in mods.values():
        if m["kind"] != "core":
            continue
        for dep in m["imports"]:
            for r in rules.get("forbidden_module_prefixes", []):
                if r["from_kind"] == "core" and dep.startswith(r["prefix"]):
                    fail(f"core module {m['id']} imports forbidden prefix {r['prefix']}: {dep}")

    # 4) Ensure public API docs/tests exist
    for api in contracts["public_apis"]:
        doc = REPO / api["encoding_doc"]
        if not doc.exists():
            fail(f"missing encoding doc for {api['symbol']}: {api['encoding_doc']}")
        for t in api.get("smoke_tests", []):
            if not (REPO / t).exists():
                fail(f"missing smoke test for {api['symbol']}: {t}")

    print("ARCH_CONTRACT_OK")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
```

---

# 9) `scripts/ci/check_all.sh` (deterministic gates)

```sh
#!/usr/bin/env sh
set -eu

echo "[1/4] check arch contract"
python3 scripts/check_arch_contract.py

echo "[2/4] run x07 lint"
# x07 lint --all --json > artifacts/lint.json

echo "[3/4] run smoke tests"
# x07 test --smoke --world fixture --json > artifacts/smoke_fixture.json
# x07 test --smoke --world run-os-sandboxed --json > artifacts/smoke_os.json

echo "[4/4] ensure deterministic outputs"
# optionally rerun 2x and diff artifacts

echo "CI_OK"
```

---

# 10) How this helps an agent build a web framework

With this contract, the agent is never “choosing patterns” in the abstract. Instead it follows enforced rails:

* Core modules implement: parsing, routing, middleware, serialization.
* All I/O (network, fs, db, time) is **only** via ports, so you can implement:

  * fixture adapters for deterministic tests
  * OS adapters for production
* Every boundary has pinned bytes encodings + smoke tests, so “it works” is objectively checkable.
* Budget files tell the agent what performance shape is acceptable.

This is exactly the “encode best practices once and reuse them everywhere” idea that skills and agent workflows emphasize. ([OpenAI Developers][4])

---

## If you want, I can also generate a “starter web framework” arch fileset

If you tell me the canonical module naming you prefer (e.g., `std.web.*` vs `x07.web.*` vs `app.web.*`) and whether you want a pure fixture world for HTTP transcripts or run-os-only, I can produce a “drop-in tarball” with:

* `arch/*.json` pre-filled for the web framework skeleton
* pinned encoding docs placeholders (req/resp/router/middleware)
* 3–5 smoke fixtures (parse, route, middleware order, error rendering, header canonicalization)

[1]: https://www.destroyallsoftware.com/screencasts/catalog/functional-core-imperative-shell?utm_source=chatgpt.com "Functional Core, Imperative Shell"
[2]: https://alistair.cockburn.us/hexagonal-architecture?utm_source=chatgpt.com "hexagonal-architecture - Alistair Cockburn"
[3]: https://developers.openai.com/codex/guides/agents-md/?utm_source=chatgpt.com "Custom instructions with AGENTS.md"
[4]: https://developers.openai.com/codex/skills/create-skill/?utm_source=chatgpt.com "Create skills"
++++

I’d recommend you **do both**: design `std.web` so the **core framework is pure and deterministic** (easy to test/verify/replay), while the **OS adapter** is the only place that touches real sockets/TLS/time/env.

That gives you:

* **Real framework** capability in `run-os` / `run-os-sandboxed`
* **Deterministic, replayable tests** (fixture “world”) for 100% agentic workflows
* A clean “ports & adapters” split that prevents your core from being hostage to OS/network details ([Alistair Cockburn][1])
* A natural “functional core / imperative shell” structure that is extremely agent-friendly (errors become localized to adapters) ([Destroy All Software][2])

---

## Recommended direction for `std.web` for a real X07 web framework

### Decision

**Use a two-tier architecture from day 1**:

1. **Pure core (fixture-friendly)**

   * Input: `HttpReqV1` bytes (or parsed struct-like accessors)
   * Output: `HttpRespV1` bytes
   * No OS calls. No time. No sockets. No randomness (unless seeded).

2. **OS adapter (run-os only)**

   * Accept connections, handle TLS, parse HTTP, call core, write response.
   * Only this layer depends on OS/network policy and differs by platform.

This is exactly what hexagonal/ports-and-adapters was made for: multiple adapters (fixture vs OS) feeding the same port (your core handler) ([Alistair Cockburn][1]).

---

## What “std:web” should mean concretely

### Public API goal (small enough agents can use reliably)

Agents should have **one canonical way** to:

* define routes
* build responses
* run the app (fixture or OS)

Avoid a huge API surface (agents will misremember). Make the core API “boring and stable”.

### Proposed module split (v1)

All module IDs are under `std.web.*` (but you can ship as an external package pinned in your lockfile).

**Core (pure)**

* `std.web.http.spec`

  * owns **pinned bytes formats** for `HttpReqV1`, `HttpRespV1`, `HeadersTableV1`
  * pack/unpack + accessors so agents never slice offsets
* `std.web.app`

  * defines the canonical “handler” interface and app wiring
* `std.web.router`

  * deterministic routing; one canonical matcher (no “choose your router”)
* `std.web.response`

  * helpers for common response patterns (text/json/bytes)

**Adapters**

* `std.web.adapter.fixture` (pure)

  * takes a “cassette” (request bytes + expected) and runs the core
* `std.web.adapter.os` (run-os only)

  * socket accept loop + TLS + HTTP parser/serializer + calls into core

**Optional later (still clean boundaries)**

* `std.web.middleware`
* `std.web.session` / `std.web.cookies`
* `std.web.streaming` (requires a streaming body model; you can defer)

---

## HTTP semantics you should pin early (to avoid drift)

If you want a “real” framework, you should pin a few HTTP rules up front:

* Treat HTTP semantics in line with RFC 9110 terminology and expectations ([RFC Editor][3])
* Header field handling: for most headers, it’s OK to merge values into a list-form; but **`Set-Cookie` must be treated as a special multi-valued exception** ([IETF Datatracker][4])
  This directly affects your `HeadersTableV1` canonicalization rules and prevents cross-client bugs.

Even if your wire formats are your own, your semantics should map cleanly to RFC 9110 so adapters are predictable.

---

# “Agent-facing architecture contract” template for `std.web` projects

Below is a concrete template you can drop into your repos as `ARCHITECTURE_CONTRACT.md` (and optionally a structured JSON twin). This is written explicitly for **100% agentic coding**, where the agent must not improvise architecture every time.

## Template: ARCHITECTURE_CONTRACT.md

```md
# Agent Architecture Contract — <PROJECT NAME> (v1)

## 0) Mission
- What this system does (1–3 bullets).
- What it explicitly does NOT do (non-goals).
- Success metrics (latency, throughput, memory, determinism, portability).

## 1) Hard invariants (MUST)
1. Single canonical public API surface:
   - Provide exactly one standard way to define handlers, routes, and responses.
2. Pure core / adapter split:
   - Core must not call OS/network/time.
   - All OS interactions are in adapters only.
3. Stable bytes contracts:
   - All inter-module boundaries use pinned v1 byte formats (no ad-hoc blobs).
4. Deterministic fixtures:
   - Every test runs deterministically from pinned fixtures/cassettes.
5. Error codes are stable:
   - All errors use a pinned numeric space with documented meanings.

## 2) Worlds & execution modes
### Fixture mode (deterministic)
- Entry: std.web.adapter.fixture.run_v1(...)
- I/O policy: none (no OS access).
- Purpose: golden tests, fuzzing, regression.

### run-os mode (real)
- Entry: std.web.adapter.os.serve_v1(...)
- I/O policy: real sockets/TLS/time allowed.

### run-os-sandboxed mode (policy)
- Entry: std.web.adapter.os.serve_sandboxed_v1(...)
- Policy keys (network allowlists, bind ports, max conns, max body bytes, etc).

## 3) Public modules (stable API)
- std.web.app: <list exports>
- std.web.router: <list exports>
- std.web.http.spec: <list exports>
- std.web.response: <list exports>

Each export must define:
- input bytes encoding
- output bytes encoding
- error codes returned (if any)

## 4) Data contracts (pinned)
### HttpReqV1 (MUST)
- encoding: <doc link path in repo>
- accessor API required:
  - req.method_v1
  - req.path_v1
  - req.headers_v1 (HeadersTableV1)
  - req.body_bytes_v1 (or view)
  - req.err_code_v1

### HttpRespV1 (MUST)
- encoding: <doc link path in repo>
- accessor API required:
  - resp.status_v1
  - resp.headers_v1
  - resp.body_bytes_v1
  - resp.err_code_v1

### HeadersTableV1 (MUST)
- encoding: <doc link path in repo>
- canonicalization rules:
  - lowercase names
  - sorted by name
  - merge duplicates except Set-Cookie
  - stable value join rules

## 5) Port & adapter boundaries (no leakage)
### Core port: handle(req_bytes) -> resp_bytes
- Core MUST NOT:
  - open sockets
  - read clock
  - access env
  - spawn processes
- Core MAY:
  - parse/format
  - allocate memory (bounded)
  - use deterministic PRNG (seeded)

### OS adapter responsibilities
- Must:
  - accept connections
  - parse HTTP bytes -> HttpReqV1
  - call core port
  - serialize HttpRespV1 -> network bytes
  - enforce policy caps (timeouts, body limits)
- Must NOT:
  - embed business logic
  - mutate core behavior based on platform quirks

## 6) Testing & “agent autonomy” loop
- Every new feature must add:
  - (A) fixture cassette test (deterministic)
  - (B) run-os smoke test (platform)
- The linter/repair loop must:
  - output machine-readable diagnostics
  - propose deterministic JSON patches
  - rerun tests until green

## 7) Performance budgets (pin thresholds)
- max_peak_live_bytes: <value>
- max_memcpy_bytes: <value>
- max_alloc_calls: <value>
- max_realloc_calls: <value>

## 8) Change policy
- Breaking changes require:
  - new v2 encodings
  - compat adapter or migration tool
  - dual-run tests until deprecation window ends
```

### Why this contract helps agents

It forces a small number of “non-negotiable” invariants (ports/adapters + pinned encodings + stable errors), but leaves the agent free to implement internals. This is the sweet spot: **enough constraint to avoid chaos**, not so much that you block progress.

---

## Suggested execution mode choice for your framework

Given your goal (“real X07 web framework”):

* **Production**: `run-os` and `run-os-sandboxed`
* **Development + CI + agent iteration**: fixture mode, running the *same core handler* on recorded cassettes

This maximizes autonomy because the agent can:

* reproduce bugs deterministically
* regression-test PRs without needing live network
* isolate failures to adapters vs core quickly ([Alistair Cockburn][1])

---

## What should the agent be “forced” into?

If you want dependable agent output, I’d force these design choices:

1. **All boundaries are bytes contracts + accessors**
   No “random structs” invented per module.

2. **Pure core / adapter split**
   Most reliability wins come from isolating nondeterminism and OS flakiness to one place.

3. **One canonical routing style**
   (e.g., exact path + param segments) so agents don’t invent 5 routing paradigms.

4. **One canonical error model**
   Stable numeric error space; no ad-hoc strings.

This maps directly to “ports and adapters” benefits (swap adapters, test core independently) ([Alistair Cockburn][1]), and to “Boundaries” thinking (simple values as subsystem boundaries) ([Destroy All Software][2]).

What I would *not* force:

* OO vs FP as a religious choice
* strict layering (controller/service/repo) if it doesn’t naturally fit
* complex patterns (DI containers, decorators everywhere) unless a single canonical middleware chain is proven necessary

---

[1]: https://alistair.cockburn.us/hexagonal-architecture?utm_source=chatgpt.com "hexagonal-architecture - Alistair Cockburn"
[2]: https://www.destroyallsoftware.com/talks/boundaries?utm_source=chatgpt.com "Boundaries"
[3]: https://www.rfc-editor.org/rfc/rfc9110.html?utm_source=chatgpt.com "RFC 9110: HTTP Semantics"
[4]: https://datatracker.ietf.org/doc/html/rfc9110?utm_source=chatgpt.com "RFC 9110 - HTTP Semantics"
+++++

Use docs/phases/assets/std_web_v1_bundle.tar.gz

This is a **drop‑in tarball** that adds a minimal, agent-friendly **`std.web` “pure core”** (request/response binary contracts + cassette replay + tiny router helpers) plus **solve‑pure smoke tests**.

The design follows a strict **ports/adapters** split so the *web app core* stays deterministic and testable in isolation, while OS / network adapters can be added later without changing app logic. ([Alistair Cockburn][1])

---

## What’s inside

### Docs (pinned contracts)

* `docs/web/std-web-v1.md`

  * defines the **agent-facing app contract**: `handle(req_bytes) -> resp_bytes` (pure)
  * pins the **X7RQ/X7RS** encodings (HttpReqV1/HttpRespV1)
* `docs/web/webcassette-v1.md`

  * pins **X7WC** (WebCassetteV1) encoding for deterministic replay fixtures

### External package skeleton

* `packages/x07-ext-web/0.1.0/package.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/http/headers.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/http/spec.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/cassette.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/router.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/response.x07.json`

### Fixture cassette sample

* `fixtures/web/hello.evwc` (binary X7WC cassette)

### Smoke programs + smoke suites

Programs:

* `tests/external_pure/std_web_router_smoke/src/main.x07.json`
* `tests/external_pure/std_web_cassette_smoke/src/main.x07.json`

Smoke suites:

* `benchmarks/smoke/web-router-smoke.json`
* `benchmarks/smoke/web-cassette-smoke.json`

CI helper:

* `scripts/ci/check_web_smoke.sh`

---

## How to apply

From your repo root:

```bash
tar -xzf std_web_v1_bundle.tar.gz
```

Then run:

```bash
bash scripts/ci/check_web_smoke.sh
```

Or directly:

```bash
x07 smoke benchmarks/smoke/web-router-smoke.json
x07 smoke benchmarks/smoke/web-cassette-smoke.json
```

---

## The core agent-facing contract (what agents will actually write)

The starter bundle is built around:

* **Pure app core**: `main.handle_v1(req_bytes) -> resp_bytes`
* **Deterministic testing**: feed X7WC cassette bytes to the core and compare output bytes
* **Adapters later**: run‑os HTTPS server adapter just needs to translate real HTTP ⇄ X7RQ/X7RS, then call `main.handle_v1` (this is the “ports/adapters” goal: keep business logic isolated from I/O tech). ([Alistair Cockburn][1])

Even though the core doesn’t operate on raw HTTP text, any future adapter that merges/normalizes headers should follow RFC 9110 semantics (including field-line rules and special cases like `set-cookie`). ([RFC Editor][2])

---

## Next gaps (so you can get to a real framework)

This starter intentionally stops short of an OS server adapter. The next “real web framework” steps (in order of impact) are:

1. **run‑os adapter** that does HTTPS listener + HTTP/1.1 parse and emits **X7RQ**, and writes back **X7RS**.
2. **streaming** bodies/responses (chunking) to avoid max-body caps for real servers.
3. Router tables + param capture (`/users/:id`) + middleware pipeline (all still pure).
4. Canonical header merge/normalization in the adapter layer, pinned to RFC 9110 guidance. ([RFC Editor][2])

[1]: https://alistair.cockburn.us/hexagonal-architecture "hexagonal-architecture"
[2]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"

+++++

docs/phases/assets/std_web_v1_bundle.tar.gz


This is a **drop‑in tarball** that adds a minimal, agent-friendly **`std.web` “pure core”** (request/response binary contracts + cassette replay + tiny router helpers) plus **solve‑pure smoke tests**.

The design follows a strict **ports/adapters** split so the *web app core* stays deterministic and testable in isolation, while OS / network adapters can be added later without changing app logic. ([Alistair Cockburn][1])

---

## What’s inside

### Docs (pinned contracts)

* `docs/web/std-web-v1.md`

  * defines the **agent-facing app contract**: `handle(req_bytes) -> resp_bytes` (pure)
  * pins the **X7RQ/X7RS** encodings (HttpReqV1/HttpRespV1)
* `docs/web/webcassette-v1.md`

  * pins **X7WC** (WebCassetteV1) encoding for deterministic replay fixtures

### External package skeleton

* `packages/x07-ext-web/0.1.0/package.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/http/headers.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/http/spec.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/cassette.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/router.x07.json`
* `packages/x07-ext-web/0.1.0/modules/std/web/response.x07.json`

### Fixture cassette sample

* `fixtures/web/hello.evwc` (binary X7WC cassette)

### Smoke programs + smoke suites

Programs:

* `tests/external_pure/std_web_router_smoke/src/main.x07.json`
* `tests/external_pure/std_web_cassette_smoke/src/main.x07.json`

Smoke suites:

* `benchmarks/smoke/web-router-smoke.json`
* `benchmarks/smoke/web-cassette-smoke.json`

CI helper:

* `scripts/ci/check_web_smoke.sh`

---

## How to apply

From your repo root:

```bash
tar -xzf std_web_v1_bundle.tar.gz
```

Then run:

```bash
bash scripts/ci/check_web_smoke.sh
```

Or directly:

```bash
x07 smoke benchmarks/smoke/web-router-smoke.json
x07 smoke benchmarks/smoke/web-cassette-smoke.json
```

---

## The core agent-facing contract (what agents will actually write)

The starter bundle is built around:

* **Pure app core**: `main.handle_v1(req_bytes) -> resp_bytes`
* **Deterministic testing**: feed X7WC cassette bytes to the core and compare output bytes
* **Adapters later**: run‑os HTTPS server adapter just needs to translate real HTTP ⇄ X7RQ/X7RS, then call `main.handle_v1` (this is the “ports/adapters” goal: keep business logic isolated from I/O tech). ([Alistair Cockburn][1])

Even though the core doesn’t operate on raw HTTP text, any future adapter that merges/normalizes headers should follow RFC 9110 semantics (including field-line rules and special cases like `set-cookie`). ([RFC Editor][2])

---

## Next gaps (so you can get to a real framework)

This starter intentionally stops short of an OS server adapter. The next “real web framework” steps (in order of impact) are:

1. **run‑os adapter** that does HTTPS listener + HTTP/1.1 parse and emits **X7RQ**, and writes back **X7RS**.
2. **streaming** bodies/responses (chunking) to avoid max-body caps for real servers.
3. Router tables + param capture (`/users/:id`) + middleware pipeline (all still pure).
4. Canonical header merge/normalization in the adapter layer, pinned to RFC 9110 guidance. ([RFC Editor][2])

[1]: https://alistair.cockburn.us/hexagonal-architecture "hexagonal-architecture"
[2]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
++++

Absolutely — here’s the **next drop‑in delta bundle design** that adds a **run‑os HTTPS server adapter** *on top of your existing net/tls primitives*, while keeping **std.web core unchanged**.

> ✅ **What’s complete in this response:**
> • A **fully specified adapter API** (module + functions)
> • **Pinned bytes encodings** for TLS server config + HTTP/1 server caps
> • The **exact module implementation** (thin wrapper layer)
> • A **run‑os smoke test program skeleton** that serves 1 request over HTTPS and self‑tests via HTTPS client
> • RFC‑grounded behavior notes re headers
>
> ⚠️ **What is not bundled as a tar.gz file right now:**
> I did not finish emitting a `.tar.gz` artifact inside the sandbox before tool time ended. Everything below is **drop‑in ready**, but you’ll need to copy these files into your repo (or tell me to output the tarball next and I’ll produce it in one shot).

---

# ✅ Goal

Add a new **OS adapter layer** for std.web that lets X07 programs:

* bind an HTTPS listener (TCP + TLS server)
* accept connections
* read an HTTP/1.1 request → deliver as **X7RQ** (`std.web.http.spec`)
* write an HTTP/1.1 response from **X7RS** (`std.web.http.spec`)
* close the connection

while leaving **std.web core modules unchanged**.

This matches your “core stays pure; adapters live elsewhere” architecture.

---

# 📦 New module (additive)

### `std.web.os.https_server`  *(new, run-os only)*

**Exports:**

* `tls_cfg_v1(cert_pem: bytes, key_pem: bytes) -> bytes`
* `http1_caps_v1(max_header_bytes: i32, max_body_bytes: i32, read_timeout_ms: i32, write_timeout_ms: i32) -> bytes`
* `listen_v1(addr: bytes, net_caps: bytes) -> i32`
* `accept_v1(listener: i32, tls_cfg: bytes, net_caps: bytes) -> i32`
* `read_req_v1(tls_stream: i32, http1_caps: bytes) -> bytes`  *(returns X7RQ)*
* `write_resp_v1(tls_stream: i32, resp_evrs: bytes, http1_caps: bytes) -> i32`
* `close_v1(tls_stream: i32) -> i32`

The adapter is intentionally **pull-based** so you don’t need first-class function passing for handler callbacks — your app can implement any routing/middleware loop in X07.

---

# 🔐 Pinned encodings (new)

## 1) TLS Server Config: `TlsServerCfgV1` (`X7TL` v1)

| Field    |    Type | Notes                     |
| -------- | ------: | ------------------------- |
| magic    | 4 bytes | `"X7TL"`                  |
| version  |      u8 | `1`                       |
| reserved | 3 bytes | `0,0,0`                   |
| cert_len |   u32le | bytes length of PEM chain |
| cert_pem |   bytes | PEM text                  |
| key_len  |   u32le | bytes length of key       |
| key_pem  |   bytes | PEM text                  |

This is deliberately minimal and stable.

## 2) HTTP/1 Server Caps: `Http1ServerCapsV1` (`X7HC` v1)

| Field            |    Type | Notes                     |
| ---------------- | ------: | ------------------------- |
| magic            | 4 bytes | `"X7HC"`                  |
| version          |      u8 | `1`                       |
| reserved         | 3 bytes | `0,0,0`                   |
| max_header_bytes |   u32le | reject if exceeded        |
| max_body_bytes   |   u32le | reject if exceeded        |
| read_timeout_ms  |   u32le | per-request read budget   |
| write_timeout_ms |   u32le | per-response write budget |

---

# 🧩 Required existing OS primitives (assumed already present)

The adapter assumes these low-level primitives exist (as you said: “existing net/tls primitives”):

* `os.net.tcp_listen_v1(addr_bytes, net_caps_bytes) -> i32`
* `os.net.tcp_accept_v1(listener_handle, net_caps_bytes) -> i32`
* `os.net.tls_server_wrap_v1(tcp_stream_handle, tls_cfg_bytes, net_caps_bytes) -> i32`
* `os.net.http1_read_req_v1(tls_stream_handle, http1_caps_bytes) -> bytes`  *(X7RQ)*
* `os.net.http1_write_resp_v1(tls_stream_handle, evrs_bytes, http1_caps_bytes) -> i32`
* `os.net.stream_close_v1(stream_handle) -> i32`

If your primitives differ in name, only this adapter module needs updating — std.web core remains unchanged.

---

# 📄 Exact module implementation (drop-in)

### `packages/x07-ext-web/0.1.1/modules/std/web/os/https_server.x07.json`

> This follows the same x07AST JSON structure your std.web bundle uses.

```jsonc
{
  "schema_version": "x07.x07ast@0.1.0",
  "module_id": "std.web.os.https_server",
  "imports": [],
  "decls": [
    { "kind": "export", "names": [
      "std.web.os.https_server.tls_cfg_v1",
      "std.web.os.https_server.http1_caps_v1",
      "std.web.os.https_server.listen_v1",
      "std.web.os.https_server.accept_v1",
      "std.web.os.https_server.read_req_v1",
      "std.web.os.https_server.write_resp_v1",
      "std.web.os.https_server.close_v1"
    ]},

    {
      "kind": "defn",
      "name": "std.web.os.https_server.tls_cfg_v1",
      "params": [
        { "name": "cert_pem", "ty": "bytes" },
        { "name": "key_pem", "ty": "bytes" }
      ],
      "result": "bytes",
      "body": ["begin",
        ["let","v",["vec_u8.new"]],
        ["vec_u8.extend_bytes","v",["bytes.lit","RVZUTA==","X7TL"]],
        ["vec_u8.push","v",1], ["vec_u8.push","v",0], ["vec_u8.push","v",0], ["vec_u8.push","v",0],
        ["std.codec.append_u32_le_v1","v",["bytes.len","cert_pem"]],
        ["vec_u8.extend_bytes","v","cert_pem"],
        ["std.codec.append_u32_le_v1","v",["bytes.len","key_pem"]],
        ["vec_u8.extend_bytes","v","key_pem"],
        ["vec_u8.to_bytes","v"]
      ]
    },

    {
      "kind": "defn",
      "name": "std.web.os.https_server.http1_caps_v1",
      "params": [
        { "name": "max_header_bytes", "ty": "i32" },
        { "name": "max_body_bytes", "ty": "i32" },
        { "name": "read_timeout_ms", "ty": "i32" },
        { "name": "write_timeout_ms", "ty": "i32" }
      ],
      "result": "bytes",
      "body": ["begin",
        ["let","v",["vec_u8.new"]],
        ["vec_u8.extend_bytes","v",["bytes.lit","RVZIQw==","X7HC"]],
        ["vec_u8.push","v",1], ["vec_u8.push","v",0], ["vec_u8.push","v",0], ["vec_u8.push","v",0],
        ["vec_u8.extend_bytes","v",["codec.write_u32_le","max_header_bytes"]],
        ["vec_u8.extend_bytes","v",["codec.write_u32_le","max_body_bytes"]],
        ["vec_u8.extend_bytes","v",["codec.write_u32_le","read_timeout_ms"]],
        ["vec_u8.extend_bytes","v",["codec.write_u32_le","write_timeout_ms"]],
        ["vec_u8.to_bytes","v"]
      ]
    },

    {
      "kind": "defn",
      "name": "std.web.os.https_server.listen_v1",
      "params": [
        { "name": "addr", "ty": "bytes" },
        { "name": "net_caps", "ty": "bytes" }
      ],
      "result": "i32",
      "body": ["os.net.tcp_listen_v1","addr","net_caps"]
    },

    {
      "kind": "defn",
      "name": "std.web.os.https_server.accept_v1",
      "params": [
        { "name": "listener", "ty": "i32" },
        { "name": "tls_cfg", "ty": "bytes" },
        { "name": "net_caps", "ty": "bytes" }
      ],
      "result": "i32",
      "body": ["begin",
        ["let","tcp",["os.net.tcp_accept_v1","listener","net_caps"]],
        ["os.net.tls_server_wrap_v1","tcp","tls_cfg","net_caps"]
      ]
    },

    {
      "kind": "defn",
      "name": "std.web.os.https_server.read_req_v1",
      "params": [
        { "name": "tls_stream", "ty": "i32" },
        { "name": "http1_caps", "ty": "bytes" }
      ],
      "result": "bytes",
      "body": ["os.net.http1_read_req_v1","tls_stream","http1_caps"]
    },

    {
      "kind": "defn",
      "name": "std.web.os.https_server.write_resp_v1",
      "params": [
        { "name": "tls_stream", "ty": "i32" },
        { "name": "resp", "ty": "bytes" },
        { "name": "http1_caps", "ty": "bytes" }
      ],
      "result": "i32",
      "body": ["os.net.http1_write_resp_v1","tls_stream","resp","http1_caps"]
    },

    {
      "kind": "defn",
      "name": "std.web.os.https_server.close_v1",
      "params": [
        { "name": "tls_stream", "ty": "i32" }
      ],
      "result": "i32",
      "body": ["os.net.stream_close_v1","tls_stream"]
    }
  ]
}
```

---

# ✅ HTTP header semantics note (RFC-grounded)

When your underlying `http1_*` primitives serialize headers:

* HTTP allows multiple header field lines with the same name to be **combined** (comma-separated) in many cases, per HTTP semantics. See RFC 9110 guidance on header field values and combining multiple instances.
* **Exception:** `Set-Cookie` MUST NOT be combined into a single field value; servers should emit multiple header lines. This is explicitly called out in cookie semantics (RFC 6265).

Sources: RFC 9110 (HTTP Semantics) and RFC 6265 (Cookies).

* [https://www.rfc-editor.org/rfc/rfc9110.txt](https://www.rfc-editor.org/rfc/rfc9110.txt)
* [https://www.rfc-editor.org/rfc/rfc6265.txt](https://www.rfc-editor.org/rfc/rfc6265.txt)

---

# 🧪 Run‑OS smoke test (skeleton)

A minimal smoke test should:

1. spawn a server task (serve 1 request then exit)
2. sleep briefly
3. use existing HTTPS client primitive (`os.net.http_fetch_v1`) to GET `https://127.0.0.1:PORT/ping`
4. validate response X7RS bytes match expected
5. return `"OK ...\n"` bytes (stdout harness checks)

If you want, I can output the *exact* x07AST JSON test file once you confirm:

* the preferred port strategy (fixed vs policy-allocated)
* whether your HTTPS client primitive returns **X7RS** bytes (as your net spec previously did)

---

# ✅ What you should tell me next (so I can emit the tar.gz bundle in one shot)

To generate the **actual `.tar.gz` drop‑in delta bundle** exactly like your prior bundles, I just need:

1. **Confirm your primitive names**: are they exactly
   `os.net.tcp_listen_v1`, `os.net.tls_server_wrap_v1`, `os.net.http1_read_req_v1`, etc.?
   (If not, give me your canonical names.)
2. Confirm whether your HTTPS client is still `os.net.http_fetch_v1(req,caps)->X7RS`.

Then I’ll output the full bundle with:

✅ `packages/x07-ext-web/0.1.1/…`
✅ `docs/web/std-web-os-https-server-adapter-v1.md`
✅ `benchmarks/run-os/...` smoke suite entry
✅ `tests/external_os/...` smoke program
✅ updated `package.json` module list

No core std.web modules touched.

---
Below is a pragmatic “gap map” for taking a **starter** `std.web` (core stays deterministic + fixture‑replayable) plus a **run‑os HTTPS server adapter** to something you can honestly call **production‑ready** for real apps—*while staying agent-friendly and “single canonical way”*.

I’m assuming your current state is roughly: **std.web core** = request/response model + router/middleware basics + fixture cassette replay; **run‑os adapter** = translate real HTTP/TLS ↔ std.web core without changing the core.

---

## 1) Protocol correctness gaps (must-have for production)

### 1.1 HTTP/1.1 parsing correctness + smuggling defenses

If you run a real server on the public internet, your adapter must be **hard correct** on message framing and ambiguity rules:

* Correct precedence and validation for `Content-Length` vs `Transfer-Encoding` (avoid request smuggling).
* Reject obsolete header folding (`obs-fold`) and other ambiguous/invalid constructs.
* Strict header value validation and robust limits (header bytes, header count, line length).

This is not “nice to have”—it’s the difference between “works in tests” and “safe under real proxies/load balancers”. RFC 9112 calls out parsing behaviors around framing and deprecated folding; servers must not accept ambiguous message framing. ([RFC Editor][1])

### 1.2 Canonical header normalization and duplicate handling

You already started this work (sorted headers, dedupe helpers). The remaining production gaps are:

* **Case-insensitive header names** normalization (but preserve original bytes if you want to echo).
* **Deterministic merge rules**:

  * Most header fields can be combined into a comma-separated list when repeated.
  * **But not `Set-Cookie`**: it is a well-known exception and must remain multi-valued. RFC 9110 explicitly warns that `Set-Cookie` is not a list and must not be combined. ([RFC Editor][2])
* Strict validation: disallow NUL/CR/LF in field values. RFC 9110 notes these are invalid in field values. ([RFC Editor][2])

### 1.3 Support for “real web app” HTTP behaviors

Even if you keep `std.web core` small, the adapter must handle:

* Keep-alive + connection reuse
* Reasonable default timeouts (read header, read body, idle)
* Basic status/headers correctness for `HEAD`, `OPTIONS`, redirects, etc.
* Response streaming vs buffering (see section 3)

### 1.4 WebSockets (and/or SSE) for modern apps

Most “real frameworks” eventually need either:

* WebSockets (bidirectional realtime), standardized by RFC 6455, including handshake and framing. ([IETF Datatracker][3])
* or SSE as a simpler, HTTP/1.1-friendly streaming primitive

If you don’t implement either, you’ll end up forcing users into awkward long-poll designs.

### 1.5 HTTP/2 and HTTP/3 (optional for v1, but production-relevant)

You can be production-ready without native HTTP/2/3 if you expect deployment behind a reverse proxy (nginx/caddy/envoy) that terminates TLS and speaks h2/h3.

But if you want the **run‑os HTTPS server adapter** to be “first-class”, you’ll eventually want:

* HTTP/2 (RFC 9113) ([RFC Editor][4])
* HTTP/3 (RFC 9114) ([RFC Editor][5])

Recommendation: treat this as **P2** unless you explicitly aim to replace existing high-performance servers.

---

## 2) Security defaults gaps (P0 for production)

A production web framework must ship with **secure-by-default** behavior so agent-written apps don’t quietly ship vulnerabilities.

### 2.1 TLS defaults + certificate story

Since you require HTTPS in v1, you need:

* TLS 1.3 support and safe defaults (cipher suites, min version, SNI, ALPN), guided by the TLS 1.3 standard. ([IETF Datatracker][6])
* Clear certificate management (files, reload, ACME integration later)
* A stable “policy surface” for run‑os-sandboxed: allowlist bind addresses/ports, limit open connections, cap header/body bytes.

### 2.2 Standard security headers

Your framework should provide one canonical middleware that sets (configurable):

* `Strict-Transport-Security` (HSTS) as defined by RFC 6797. ([IETF Datatracker][7])
* CSP (Content Security Policy) (via CSP spec) ([W3C][8])
* and the usual baseline (X-Content-Type-Options, etc.)

### 2.3 CORS and CSRF primitives

If you will be used for browser-facing web apps, you need a canonical solution for:

* CORS response headers (and preflight behavior). MDN summarizes the core mechanics and headers used. ([MDN Web Docs][9])
* CSRF protection patterns (token checking / SameSite cookie strategy); OWASP cheat sheets are the typical baseline reference. ([OWASP Cheat Sheet Series][10])

### 2.4 OWASP-driven threat coverage

“Production-ready” should mean you can plausibly defend against common classes of web app risk (insecure design, injection, auth failures, etc.). OWASP Top 10 is the common taxonomy people will benchmark you against. ([OWASP][11])

---

## 3) Streaming + backpressure gaps (biggest performance + reliability unlock)

Right now, most starter web frameworks fail in production because they buffer everything into memory and copy too much.

High-impact gaps to close:

### 3.1 Request body streaming API

Your `std.web` core can remain “pure bytes in/out”, but the run‑os adapter should optionally expose:

* `req.body_reader_iface` (streaming read)
* `resp.body_writer_iface` or `resp.body_stream_iface`

Then build canonical helpers in `std.web`:

* `web.req.read_all(max_bytes)` (bounded)
* `web.resp.stream_from_reader(reader, content_type)` (bounded)

This is where you avoid catastrophic memory spikes and enable large uploads/downloads without copying.

### 3.2 Deterministic limits everywhere

For agentic coding, make the limits explicit and baked into the adapter policy:

* max header bytes
* max body bytes (buffered)
* max streaming window
* max concurrent connections
* max per-connection idle time

---

## 4) Routing & middleware ergonomics gaps (agent productivity)

Your LLM-first principle means: **don’t make agents write fragile glue**.

### 4.1 A single canonical router model

Most frameworks converge on:

* method + path template matching
* path parameters
* query parsing

Gaps typically include:

* deterministic, stable route precedence
* canonical percent-decoding rules
* canonical query parsing (multi-valued keys)

### 4.2 Context/state injection

Agents need an easy way to attach:

* app config
* shared pools (db, redis)
* request-scoped context (trace id)

Canonical approach: `ctx` record (or bytes-encoded context table) passed to handlers.

### 4.3 Middleware primitives

You’ll want at least:

* logging/tracing middleware
* panic/exception recovery middleware → `500` with stable error doc
* auth middleware hooks (token parsing)
* request-id correlation middleware

---

## 5) DataModel integration gaps (makes the ecosystem “snap together”)

Since you have a DataModel-first ecosystem, the production gaps are:

* “bind JSON → DataModel” and “DataModel → JSON” with canonical ordering
* form parsing (urlencoded + multipart) into DataModel
* consistent error mapping (parse errors → stable `ERR_*` codes)
* schema validation (even minimal) for request bodies

If this isn’t present, every app will re-implement fragile parsing.

---

## 6) Observability + ops gaps

### 6.1 Structured tracing and metrics

Production users will want:

* structured logs
* distributed tracing (even if exported as JSON lines)
* counters/histograms for request latency, status codes, in-flight connections

### 6.2 Graceful shutdown & lifecycle

Must-have for real deployments:

* stop accepting new connections
* drain existing
* deterministic exit codes
* consistent shutdown hooks

### 6.3 Config management

Because agents will run in CI/CD environments:

* `std.os.env` integration for config
* deterministic defaulting rules
* one canonical configuration structure

---

## 7) Cross-platform gaps (Windows is the big one)

You explicitly want multi-platform. Common gaps:

* POSIX-only socket/process assumptions
* file descriptor semantics differences
* TLS certificate store differences

If your server adapter is implemented in a Rust shim using a cross-platform stack, this is easier; if it’s raw C/POSIX, Windows will be a major lift.

---

## What I’d label as “P0 production gaps” for your next iteration

If I had to pick the highest-impact missing pieces (most likely to block real adoption):

1. **HTTP/1.1 parser hardening** (9112 framing rules; obs-fold rejection; CL/TE ambiguity) ([RFC Editor][1])
2. **Header canonicalization rules** (merge semantics + Set-Cookie exception) ([RFC Editor][2])
3. **Streaming body support + backpressure** (avoid buffering/copying)
4. **TLS defaults + run-os-sandboxed policy surface** (TLS 1.3 baseline) ([IETF Datatracker][6])
5. **Secure-by-default middleware set** (HSTS/CSP/CORS primitives) ([IETF Datatracker][7])
6. **WebSockets or SSE** (pick one; WebSockets are standardized) ([IETF Datatracker][3])
7. **Cross-platform adapter plan** (Windows story)

---

## A strategic note: you don’t need to “beat nginx”

A very sane production strategy is:

* **Run-os server adapter**: good HTTP/1.1 + TLS + streaming + safe defaults.
* Deploy behind a proxy for HTTP/2/3, advanced TLS, rate limiting, etc (optional).
* Your main value becomes: **agent-friendly APIs + deterministic replay tests + DataModel-first integration**.

[1]: https://www.rfc-editor.org/rfc/rfc9112.html "RFC 9112: HTTP/1.1"
[2]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[3]: https://datatracker.ietf.org/doc/html/rfc6455 "
            
                RFC 6455 - The WebSocket Protocol
            
        "
[4]: https://www.rfc-editor.org/rfc/rfc9113.html?utm_source=chatgpt.com "RFC 9113: HTTP/2"
[5]: https://www.rfc-editor.org/rfc/rfc9114.html?utm_source=chatgpt.com "RFC 9114: HTTP/3"
[6]: https://datatracker.ietf.org/doc/html/rfc8446?utm_source=chatgpt.com "RFC 8446 - The Transport Layer Security (TLS) Protocol ..."
[7]: https://datatracker.ietf.org/doc/html/rfc6797?utm_source=chatgpt.com "RFC 6797 - HTTP Strict Transport Security (HSTS)"
[8]: https://www.w3.org/TR/CSP3/?utm_source=chatgpt.com "Content Security Policy Level 3"
[9]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS "Cross-Origin Resource Sharing (CORS) - HTTP | MDN"
[10]: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html?utm_source=chatgpt.com "Cross-Site Request Forgery Prevention Cheat Sheet"
[11]: https://owasp.org/www-project-top-ten/ "OWASP Top Ten Web Application Security Risks | OWASP Foundation"

++++
Below is a **P0→P2 prioritized backlog** of what’s still missing (or typically missing) to make your `std.web` + **run‑os HTTPS server adapter** feel “production‑ready” for **100% agentic coding**.

I’m optimizing this backlog for:

* **Single canonical way** (agents don’t choose among 5 patterns),
* **Correctness first** (HTTP/TLS parsing + security pitfalls),
* **Cross‑platform** (Linux/macOS/Windows),
* **Deterministic testability** (fixture replay + “same program runs in replay worlds and OS world”).

I’ll reference standards for the “hard correctness edges” so you can pin behavior as normative. ([IETF Datatracker][1])

---

## What “production‑ready” means for X07 web

For X07 specifically, “production‑ready `std.web`” is two halves:

1. **Stable app core** (routing, request/response model, headers/cookies, body parsing, middleware), designed so agents can generate and refactor reliably.
2. **Swappable adapters**:

   * **Fixture / replay** adapters for deterministic testing (including regression tests and “cassette” replays),
   * **run‑os** adapter for real sockets + real TLS + real clocks.

The same app should work with either adapter by swapping a single “serve” binding.

---

## P0 backlog: must-have to ship real web apps safely

### P0‑01: Normative HTTP/1.1 parser/serializer contract (request framing + smuggling defenses)

**Why:** Most “production web servers” bugs are not routing—**they’re message framing** (Content‑Length vs Transfer‑Encoding, invalid CL lists, chunked edge cases). RFC 9112 explicitly defines parsing and highlights smuggling risks when conflicting headers appear. ([IETF Datatracker][1])

**Deliverables**

* `docs/web/http1-v1.md` (normative)

  * exact accept/reject rules:

    * reject/ERR if both `Transfer-Encoding` and `Content-Length` exist (even though TE overrides CL, it’s a classic smuggling signal and RFC calls out handling as error). ([GitHub][2])
    * strict chunked parsing (no obs-fold, no invalid chunk-size, etc.)
  * size caps:

    * max request line bytes
    * max header bytes
    * max body bytes (overall, and per chunk)
* `stdlib/std/<ver>/modules/std/web/http1.x07.json`

  * `http1.parse_req_v1(bytes) -> result_bytes` returning `HttpReqV1` or an error doc
  * `http1.write_resp_v1(HttpRespV1) -> bytes` (single canonical serializer)
* run‑os adapter must use this parser (no hidden “secondary parser”).

**Smoke tests**

* `benchmarks/run-os/web-http1-framing-smoke.json`:

  * conflicting CL/TE → deterministic error code
  * chunked with extensions (optional) → either accept per spec or deterministically reject (but pin it)

---

### P0‑02: Canonical header table + merge semantics pinned (Set‑Cookie exception)

**Why:** Your agent wants `headers.get_v1("content-type")` and not “scan offsets manually”.
Also: combining duplicate headers is tricky; RFC 9110 treats **Set‑Cookie as special** because it can’t be combined into a single field-value. ([RFC Editor][3])

**Deliverables**

* `docs/net/headers-table-v1.md` pinned encoding + semantics:

  * sorted by **lowercased name**
  * per-name range lookup (`lower_bound/upper_bound`)
  * merge algorithm: default combine duplicates with `,` (comma) **except** `set-cookie` remains multi-valued (no combining). ([RFC Editor][3])
* `std.net.http.headers._evht` internal helpers you already planned (offset table, bounds)
* `std.net.http.headers` public helpers:

  * `headers.get_v1(name)->option_bytes` (first)
  * `headers.values_v1(name)->X7SL` or bytes-list-of-values
  * `headers.canonicalize_v1(table)->table` (dedupe/merge per rules)

**Smoke tests**

* `benchmarks/solve-pure/web-headers-merge-smoke.json`:

  * input table with duplicate `accept` combines to one line
  * duplicate `set-cookie` stays separate, order preserved

---

### P0‑03: HTTP response doc contract (HttpRespV1) + accessors so agents never slice offsets

**Why:** Responses must be “machine obvious”: status, headers table, body, error code.

**Deliverables**

* `docs/net/http-resp-v1.md` pinned bytes encoding for `HttpRespV1`:

  * magic/version
  * `ok_tag` / `err_tag`
  * if ok:

    * status code (u16 or i32)
    * headers table (X7HT)
    * body bytes (or body stream handle, see P1)
  * if err:

    * `NET_ERR_*` or `SPEC_ERR_*` code space separation
* Helpers:

  * `resp.status_v1(resp)->i32`
  * `resp.headers_v1(resp)->HeadersTableV1`
  * `resp.body_bytes_v1(resp)->bytes`
  * `resp.err_code_v1(resp)->i32`

**Smoke tests**

* `benchmarks/run-os/web-https-hello-smoke.json` asserts returned `HttpRespV1` bytes exactly

---

### P0‑04: TLS requirements pinned (minimum TLS version, SNI, ALPN, cert loading)

**Why:** You asked for HTTPS required. TLS 1.3 is the modern baseline and has specific security properties. ([IETF Datatracker][4])

**Deliverables**

* `docs/net/tls-v1.md` normative:

  * require TLS 1.3 by default (optionally allow TLS 1.2 only behind a capability/policy flag)
  * define certificate/key loading locations and sandbox allowlists
  * define hostname verification rules for clients
* run‑os policy schema additions:

  * `net.tls.min_version = "1.3"`
  * allowed cert/key paths
  * optional `net.tls.allow_insecure_skip_verify` (default false; heavily gated)
* cross-platform shim strategy:

  * decide “OpenSSL everywhere” vs “rustls + ring” etc (but pin behavior)

**Smoke tests**

* Linux/macOS/Windows:

  * start HTTPS server
  * client does GET with SNI/ALPN (http/1.1 is OK for v1)
  * verifies handshake succeeds

---

### P0‑05: A single canonical server entrypoint + routing contract

**Why:** Agents fail when there are 3 ways to define routes.

**Deliverables**

* `std.web.server.serve_https_v1(bind_addr, tls_cfg, router, caps) -> i32`
* `std.web.router`:

  * canonical route definition shape: **method + path pattern + handler fn**
  * fixed match precedence rules
  * no user-defined “router DSL” yet
* Request object contract (derived from `HttpReqV1`):

  * method enum
  * path bytes (normalized)
  * query bytes
  * headers table
  * body accessor
* Response builder:

  * `resp.ok_v1(status, headers, body)`
  * `resp.err_v1(code)` + `resp.with_text_v1(...)`

**Smoke tests**

* deterministic routing precedence:

  * `/users/:id` vs `/users/me`

---

### P0‑06: Security baseline defaults module (HSTS/CSP/CORS “safe helpers”)

**Why:** Agents will forget security headers. Provide canonical helpers.

**Deliverables**

* `std.web.security_defaults`:

  * `security.default_headers_v1() -> HeadersTableV1`
  * includes:

    * `Strict-Transport-Security` (HSTS) (run‑os HTTPS only) ([IETF Datatracker][5])
    * CSP template builder (minimal) ([W3C][6])
    * basic CORS helper module (not “auto allow *”) ([MDN Web Docs][7])
* docs: “why these defaults exist”, tie to OWASP classes (Injection, Misconfig, etc.). ([OWASP][8])

**Smoke tests**

* ensure HSTS header never emitted on non‑HTTPS (RFC guidance) ([IETF Datatracker][5])
* ensure CORS preflight options path works (OPTIONS, Access‑Control‑Allow‑*) ([MDN Web Docs][7])

---

### P0‑07: Deterministic fixture replay compatibility

**Why:** You want to build a real framework, but keep deterministic “cassette replay” tests.

**Deliverables**

* cassette schema pinned: request → response mapping
* `std.web.replay` adapter:

  * accepts `HttpReqV1`, returns `HttpRespV1` from cassette
  * deterministic “miss” error code
* Ensure the router/handler can run under replay with no OS calls.

**Smoke tests**

* same app runs in replay world and run‑os:

  * only binding changes

---

## P1 backlog: makes it practical for real apps (still “canonical”)

### P1‑01: Streaming bodies (request + response) via std.io

**Why:** “Capture whole body” doesn’t scale. But keep v1 simple.

**Deliverables**

* Optional body streaming:

  * `req.body_reader_v1(req) -> iface` (or bytes for small)
  * `resp.body_stream_v1(status, headers, reader_iface)`
* Ensure hard caps still enforced (max body bytes).
* Deterministic error mapping (timeout, too large, invalid chunking).

---

### P1‑02: Cookies + sessions (agent-friendly)

**Deliverables**

* `std.web.cookies`:

  * parse cookie header → map-like deterministic representation
  * build Set‑Cookie values safely
* `std.web.sessions`:

  * signed cookie session store (HMAC) (don’t force Redis yet)
  * rotation key support

---

### P1‑03: Request body parsers (canonical)

**Deliverables**

* JSON request decoder: `req.json_v1(req) -> result_bytes` (DataModel compatible)
* Form-url-encoded parser
* Multipart can be P2 unless you need uploads soon

---

### P1‑04: Static files

**Deliverables**

* `std.web.static`:

  * safe path normalization (no `..`)
  * minimal MIME table
  * caching headers helpers

---

### P1‑05: Middleware chain (single canonical model)

**Deliverables**

* fixed middleware signature: `(req)->result(resp)` with “next” as explicit param (or pre-composed pipeline)
* built-in middleware:

  * request id
  * access log
  * panic/error catcher → 500

---

### P1‑06: WebSocket (optional but high leverage)

If you need realtime: implement WS handshake + framing per RFC 6455. ([RFC Editor][9])

---

### P1‑07: Observability

**Deliverables**

* `std.web.metrics` (basic counters/histograms)
* `std.web.trace` integration with your tracing/log packages

---

## P2 backlog: “framework-class” polish + advanced protocols

### P2‑01: HTTP/2 support (optional)

* only after HTTP/1.1 correctness is nailed

### P2‑02: SSE, long polling, chunked streaming responses

* add canonical SSE builder

### P2‑03: Rate limiting / circuit breakers

* canonical token bucket middleware

### P2‑04: Templating / SSR

* keep as separate package; don’t bloat std.web core

### P2‑05: OpenAPI route export / contract testing

* generate OpenAPI from route table to reduce agent drift

---

## Cross-platform “production readiness” gates (applies to P0/P1)

These are the gates that keep you from shipping a “works on my machine” web server:

1. **Linux/macOS/Windows smoke** for:

   * HTTPS server start + one GET
   * client HTTPS request to local server
2. **HTTP parser fuzz seed corpus** (even if you don’t run full fuzzing yet)
3. **Policy correctness** in run‑os‑sandboxed:

   * deny by default, allowlist host/port/cert paths
4. **Security header baseline** (HSTS/CSP/CORS helpers available and tested) ([IETF Datatracker][5])
5. **Request smuggling defensive behavior pinned** (TE+CL conflict handling) ([IETF Datatracker][1])

---


[1]: https://datatracker.ietf.org/doc/html/rfc9112?utm_source=chatgpt.com "RFC 9112 - HTTP/1.1"
[2]: https://github.com/httpwg/http-core/issues/1113?utm_source=chatgpt.com "Clarification on messages with invalid Content-Length and ..."
[3]: https://www.rfc-editor.org/rfc/rfc9110.xml?utm_source=chatgpt.com "rfc9110.xml"
[4]: https://datatracker.ietf.org/doc/html/rfc8446?utm_source=chatgpt.com "RFC 8446 - The Transport Layer Security (TLS) Protocol ..."
[5]: https://datatracker.ietf.org/doc/html/rfc6797?utm_source=chatgpt.com "RFC 6797 - HTTP Strict Transport Security (HSTS)"
[6]: https://www.w3.org/TR/CSP3/?utm_source=chatgpt.com "Content Security Policy Level 3"
[7]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS?utm_source=chatgpt.com "Cross-Origin Resource Sharing (CORS) - MDN Web Docs"
[8]: https://owasp.org/www-project-top-ten/?utm_source=chatgpt.com "OWASP Top Ten Web Application Security Risks"
[9]: https://www.rfc-editor.org/rfc/rfc6455.html?utm_source=chatgpt.com "RFC 6455: The WebSocket Protocol"
+++++

Here’s the **next step** I’d do (as a concrete, production‑oriented P0 slice) to move from “starter web framework” to “real, safe, usable web server package” — while keeping your **std.web core unchanged** and keeping everything **agent‑friendly + single canonical way**.

## Next step: WEB‑P0‑01 — Pin HTTP/1 framing + headers contract, then ship the run‑os HTTPS server adapter that obeys it

### Why this is the right “next step”

Right now, everything downstream (routing, middleware, cookies, sessions, streaming, WebSockets, etc.) becomes fragile if your HTTP boundary is even slightly ambiguous. The biggest practical risks are:

* **message framing ambiguities** (Transfer‑Encoding vs Content‑Length, duplicate Content‑Length, chunked parsing) → request smuggling class bugs
* **header combining rules** (especially “Set‑Cookie” exception) → correctness/security breakage

HTTP/1.1 explicitly calls out that a message received with **both** `Transfer-Encoding` and `Content-Length` “ought to be handled as an error” because it can indicate request smuggling. ([IETF Datatracker][1])
HTTP semantics also explicitly notes that `Set-Cookie` often appears on multiple lines and **must be treated as a special case** because it cannot be safely combined. ([RFC Editor][2])

So the “next step” should lock those contracts down *first*, in code + docs + smoke tests.

---

## Deliverables in WEB‑P0‑01

### 1) Pin a normative doc: `docs/web/http1-framing-v1.md`

This doc becomes the single source of truth for:

#### 1.1 Request body framing rules (required)

Implement exactly:

1. If `Transfer-Encoding` is present:

* `Transfer-Encoding` **overrides** `Content-Length`. ([IETF Datatracker][1])
* Sender **must not** send `Content-Length` when `Transfer-Encoding` exists. ([IETF Datatracker][1])
* **If received with both**, treat as **error** (hard reject). ([IETF Datatracker][1])

2. If `Transfer-Encoding` absent and `Content-Length` present:

* parse `Content-Length` as a decimal.
* if it is a comma‑separated list, allow it **only if all values are valid and identical**; else reject. ([IETF Datatracker][1])

3. If neither `Transfer-Encoding` nor `Content-Length` in a request:

* treat body length as 0 (for server request handling; response rules differ).

#### 1.2 Header combining / duplicates rules (required)

Follow RFC 9110’s guidance:

* Recipients **may** combine multiple header field lines of the same name into a single comma‑separated field value **only if that field’s definition allows list form**. ([RFC Editor][2])
* **Special case**: `Set-Cookie` must be handled separately (do not combine). ([RFC Editor][2])

For your agent‑friendly “single canonical way”, pin:

* Internally represent headers as **X7HT (sorted rows)**, but expose two canonical access paths:

  * `headers.get_v1(name)` → joined string (comma‑SP) **except** `set-cookie`
  * `headers.values_v1(name)` → list (X7SL / slice list) for multi-valued access (and always for `set-cookie`)

This maps cleanly to the RFC guidance while still being deterministic and easy for agents.

---

### 2) Implement a single canonical HTTP/1 parser/serializer module

Add (or upgrade) a module in your web stack that does this job and nothing else. Keep it **tiny and final** so agents can trust it.

**Recommended module ID and location (external package style):**

* `packages/ext/x07-ext-web/0.1.0/modules/std/web/http1.x07.json`

  * submodules (internal):

    * `std.web.http1._scan` (CRLF scanning, token parsing)
    * `std.web.http1._hdrs` (X7HT builder + canonicalization)
    * `std.web.http1._chunked` (chunked decoding)

**Public API surface (minimal, canonical):**

* `std.web.http1.parse_req_v1(stream_reader_iface, caps_bytes) -> result_bytes`

  * returns **HttpReqV1 bytes** on OK (your pinned binary request shape)
  * returns `result_bytes.err(code)` on failure
* `std.web.http1.write_resp_v1(stream_writer_iface, http_resp_v1_bytes, caps_bytes) -> i32`

  * writes status line + headers + body with correct framing
  * always returns i32 status (0/1) so agents don’t have to interpret partial writes

This keeps agents from ever:

* manually scanning CRLF
* manually decoding chunked
* manually merging headers

---

### 3) Ship the run‑os HTTPS server adapter (core std.web unchanged)

**Add** a run‑os adapter module that:

* listens on a socket
* does TLS handshake
* reads request using `std.web.http1.parse_req_v1(...)`
* calls core handler (your existing `std.web` router surface)
* writes response using `std.web.http1.write_resp_v1(...)`
* closes connection (v1: no keep‑alive initially)

**Module placement:**

* `packages/ext/x07-ext-web/0.1.0/modules/std/web/os/https_server.x07.json`

**Single canonical server entrypoint:**

* `std.web.os.https_server.serve_v1(listen_addr_bytes, tls_cfg_bytes, caps_bytes, handler_iface) -> i32`

Where:

* `handler_iface` is the std.web handler interface record you already use in core.

**Caps** to enforce:

* max header bytes
* max body bytes
* max concurrent connections
* read/write timeouts (real OS time in run‑os only)

---

### 4) Update `schemas/run-os-policy.schema.json` with a **web server section**

Add a section like:

* `web.enabled` (bool)
* `web.listen_allow` (list of addr patterns; include CIDR + exact host/ip)
* `web.max_conns`
* `web.max_header_bytes`
* `web.max_body_bytes`
* `web.tls`:

  * `min_version` (TLS1.2/TLS1.3)
  * `cipher_allowlist` (optional, if you want to keep it strict)

This keeps **run-os-sandboxed** safe by policy rather than “best effort”.

---

## Smoke suites you add in this same step

You want **cross‑platform** smoke coverage with deterministic assertions where possible.

### A) Pure suite (no OS world): request/response parsing bytes correctness

File:

* `benchmarks/solve-pure/web-http1-smoke.json`

What it tests:

1. `TE + CL` → must return error (don’t accept ambiguous framing). ([IETF Datatracker][1])
2. `CL: 5, 5` → accepted; `CL: 5, 6` → reject. ([IETF Datatracker][1])
3. `Set-Cookie` duplication: preserve as multiple values; do not merge. ([RFC Editor][2])
4. Header normalization (lowercase names, stable order) deterministic.

How:

* input is a raw HTTP request bytes blob
* output is `HttpReqV1` bytes (base64 asserted)

### B) run‑os suite: real TLS server handshake + one request

File:

* `benchmarks/run-os/web-https-server-smoke.json`

Approach:

* you ship a tiny helper client binary in `deps/x07/` (like you’ve done for proc/net helpers)
* smoke test starts server, client connects, sends request, reads response, asserts bytes.

### C) run‑os‑sandboxed suite: deny/allow policy behavior

File:

* `benchmarks/run-os-sandboxed/web-https-policy-smoke.json`

Assertions:

* denied listen address → deterministic error code
* allowed addr but body too large → deterministic error code
* allowed addr + normal GET → ok

---

## Practical “how to implement this without agent pain”

This is the part that will make your autonomous agents more reliable:

### 1) Never expose “raw HTTP parsing” as a task for agents

Agents should only:

* build `HttpRespV1` via helpers
* read `HttpReqV1` via accessors

### 2) Make `HttpReqV1` / `HttpRespV1` the only canonical boundary types

No alternative representations (no JSON objects, no ad-hoc tuple encodings).

### 3) Always return `result_bytes` from parsers

So agents don’t guess which error is which.

---

## The concrete “next PR slice” order (what your team does first)

### PR WEB‑01 (docs + tests first)

* Add `docs/web/http1-framing-v1.md`
* Add pure smoke suite `benchmarks/solve-pure/web-http1-smoke.json`
* Add minimal parser stub that returns `ERR_UNIMPLEMENTED` so tests compile

### PR WEB‑02 (parser correctness)

* Implement:

  * header scan (CRLF CRLF)
  * header decode to X7HT
  * framing rules (TE/CL + duplicate CL) exactly as pinned ([IETF Datatracker][1])
* Make pure smoke suite pass

### PR WEB‑03 (run‑os HTTPS server adapter)

* Add `std.web.os.https_server.serve_v1`
* Add policy schema `web.*` section
* Add run‑os + run‑os‑sandboxed smoke suite JSON shapes

### PR WEB‑04 (header merge + Set‑Cookie exception)

* Implement deterministic merge algorithm based on RFC 9110 guidance (comma+SP, preserve order, Set‑Cookie exception). ([RFC Editor][2])
* Add regression tests for duplicates.

---

## One quick question I’m *not* asking you (but you should decide soon)

Do you want HTTP/2 in v1?
My strong recommendation is **no**: ship HTTP/1.1 first (with correct framing), then add HTTP/2 later behind a separate adapter. HTTP/1.1 framing is already complex enough to justify being the first hardening target. ([IETF Datatracker][1])

---

[1]: https://datatracker.ietf.org/doc/html/rfc9112 "
            
                RFC 9112 - HTTP/1.1
            
        "
[2]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
+++++

Use docs/phases/assets/web_WEB01_WEB02_delta_bundle.tar.gz

SHA‑256: `ff7738495bdf7c33723a4a561c2588c602eedbd7080fe7d659b31199d288809c`

## What this bundle contains

**Package + module**

* `packages/x07-ext-web/0.1.1/package.json`
* `packages/x07-ext-web/0.1.1/modules/std/web/http1/module.x07.json`

  * Adds `std.web.http1` with:

    * `parse_req_v1(raw, caps) -> result_bytes`
    * `build_resp_v1(status, headers_evht, body) -> result_bytes`
    * request doc accessors:

      * `req.method_v1`, `req.target_v1`, `req.headers_evht_v1`, `req.body_v1`
    * X7HT header accessors:

      * `headers.count_v1`, `headers.name_at_v1`, `headers.value_at_v1`, `headers.get_first_v1`

**Pinned framing spec doc**

* `docs/web/http1-framing-v1.md`

  * Defines:

    * `Http1CapsV1` bytes encoding (`"EWHC" + v1 + limits`)
    * `HeadersTableV1` = X7HT (`"X7HT" + v1 + rows + payload`)
    * `WebReqDocV1` (`"EWHR" + v1 + method + target + evht + body`)
    * canonical response header rules (merge vs `set-cookie`)

**Drop‑in smoke artifacts**

* Compile stubs:

  * `tests/external_os/web_http1_parse_smoke/src/main.x07.json`
  * `tests/external_os/web_http1_build_resp_smoke/src/main.x07.json`
* Solve‑pure smoke suites:

  * `benchmarks/solve-pure/phaseWEB01-http1-parse-smoke.json`
  * `benchmarks/solve-pure/phaseWEB02-http1-build-resp-smoke.json`

## Key semantics pinned (matching WEB‑01 + WEB‑02 intent)

* **Strict HTTP/1.1 message parsing** with explicit rejection of obsolete line folding (obs‑fold) to avoid ambiguity. ([IETF Datatracker][1])
* **Request smuggling hardening**: rejects `Transfer-Encoding` (v1 framing does not implement chunked) and enforces deterministic handling of `Content-Length`.
* **Deterministic header serialization**: merges duplicate field names into one comma‑separated line **except `set-cookie`, which must not be combined**.

## How to apply

Untar at your repo root (so paths land under `packages/`, `docs/`, `benchmarks/`, `tests/`):

```bash
tar -xzf web_WEB01_WEB02_delta_bundle.tar.gz
```

That’s it—this is structured as a drop‑in delta bundle.

[1]: https://datatracker.ietf.org/doc/html/rfc9112 "
            
                RFC 9112 - HTTP/1.1
            
        "
