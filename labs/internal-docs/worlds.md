# Worlds

X07 compilation and execution always happens in a **world** (a capability profile).

The authoritative machine registry is `crates/x07-worlds/src/lib.rs` (`WorldId` + `WorldCaps`).
Compiler/linter world → feature-flag mapping (fs/rr/kv + sandboxed unsafe/ffi defaults) is `crates/x07c/src/world_config.rs`.

## Tiers

- **Eval worlds** (`solve-*`): deterministic, capability-limited, and permitted in benchmark suites.
- **Standalone worlds** (`run-os*`): non-deterministic by design and never permitted in benchmark suites.

## World list

| World | Tier | Deterministic | Capabilities (high level) |
| --- | --- | --- | --- |
| `solve-pure` | eval | yes | pure compute only |
| `solve-fs` | eval | yes | fixture filesystem (read-only) |
| `solve-rr` | eval | yes | fixture request/response (no real network) |
| `solve-kv` | eval | yes | seeded deterministic key/value store |
| `solve-full` | eval | yes | `solve-fs` + `solve-rr` + `solve-kv` |
| `run-os` | standalone | no | real OS access (fs/env/time/process/net) + `unsafe` + `extern "C"` |
| `run-os-sandboxed` | standalone | no | same surface as `run-os`, but restricted by a policy file (`schemas/run-os-policy.schema.json`) |

## Capability gating rules

- **Suite allowlist**: deterministic benchmark tooling refuses non-eval worlds for `x07 bench` suites.
- **Compiler gating**:
  - `os.*` builtins are standalone-only and are rejected in `solve-*` worlds at compile time.
  - Phase H4 “systems” features are standalone-only:
    - `unsafe { ... }` blocks
    - raw pointers (`ptr_*`)
    - `extern "C"` declarations and calls

## Runners

- Deterministic execution: `crates/x07-host-runner/` (only `solve-*` worlds)
- Standalone OS execution: `crates/x07-os-runner/` (only `run-os*` worlds)

Example (deterministic):

`cargo run -p x07-host-runner -- --program program.x07.json --world solve-pure --input case.bin`

Example (standalone OS):

`cargo run -p x07-os-runner -- --program program.x07.json --world run-os-sandboxed --policy policy.json`
