# Prompt 8 — State machines as data + arch contracts (gen + drift + enforcement)

```text
You are an X07 coding agent building `x07-sm-arch-contracts-smoke`.

GOAL
Build a small “workflow engine” project where the core control-flow is defined as a state machine spec.
The toolchain must generate the implementation, and CI must detect drift.

The project must use:
- `x07 sm gen` (generate a deterministic step function + tests)
- `x07 arch check` with `contracts_v1.sm` enabled (validate the SM index + require generated outputs up to date)
- `budget.scope_from_arch_v1` for step-level caps (pinned under `arch/budgets/`)

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-sm-arch-contracts-smoke
cd x07-sm-arch-contracts-smoke
x07 init

STATE MACHINE SPEC
1) Write an SM spec file (JSON) under `arch/sm/` that models a simple lifecycle:
   - states: `init`, `ready`, `running`, `done`, `error`
   - events: `start`, `tick`, `finish`, `fail`
   - transitions with stable error codes for invalid transitions
2) Run `x07 sm gen --input <spec> --out gen/sm --write`.
3) Run the generated tests via `x07 test --manifest gen/sm/tests.manifest.json`.

ARCH CONTRACTS
1) Create `arch/sm/index.x07sm.json` (`schema_version: "x07.arch.sm.index@0.1.0"`) referencing your spec file(s).
2) Create an `arch/manifest.x07arch.json` that enables `contracts_v1.sm` with:
   - `index_path` pointing at `arch/sm/index.x07sm.json`
   - `gen_dir` pointing at `gen/sm`
   - `require_gen_uptodate=true`
3) Run `x07 arch check --write-lock` and commit the lock.

BUDGETS
1) Define a budget profile under `arch/budgets/profiles/` for the generated `step_v1` hot path.
2) In your application entrypoint, wrap calls into the generated `step_v1` with `budget.scope_from_arch_v1`.

APPLICATION
1) Implement an entrypoint that:
   - accepts an event stream from input bytes (newline-delimited event names is fine)
   - feeds events into the generated machine
   - outputs a canonical report bytes value (including final state and counts)
2) The program must be deterministic and must run under `solve-pure` (no OS calls).

TESTS
- Include a deterministic test suite with multiple event streams (success path, invalid transition path, error path).
- CI must fail if:
  - generated outputs drift (`x07 sm gen --check`)
  - arch contracts drift (`x07 arch check` errors)
```

