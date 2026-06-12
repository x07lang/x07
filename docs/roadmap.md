# X07 roadmap (next 12 months)

## Goal

Make X07 the deterministic, certifiable execution substrate for agent-written
software: code that agents (or transpilers) produce runs sandboxed, budgeted,
replayable, and provable. Direct authoring by agents and humans stays a
supported surface, with further language investment gated on measured
evidence (see the eval gate below).

## Positioning (updated 2026-06)

The trust half of X07 — deterministic worlds, budgets, record/replay,
structured diagnostics with quickfix coverage, XTAL, certification — grows
more valuable as code generation gets cheaper, because verification and
review become the bottleneck. The language-surface half is the adoption
barrier (agent case studies and the 2026-06 strategic review agree). The
roadmap therefore leads with the substrate, and treats language-surface work
as gated, evidence-driven investments.

## The eval gate

`labs/agent-eval/` holds a comparative benchmark (agents solving identical
tasks in X07 vs Python/Rust) with a pilot result and a scaled runbook with a
predeclared decision rule. The scaled run decides:

- pass → prioritize RFC 0002 (expressiveness floor: records, enums + match,
  string, f64) and re-run the eval after.
- fail → park direct-authoring guidance; X07 proceeds substrate-first
  (transpile target + verification + sandboxing), and language-surface work
  stops after the x07text projection.

## Track 1 - Language and toolchain

- agent DX: did-you-mean suggestions on unknown symbols, fuzzy `x07 doc`
  lookup, behavioral summaries for stdlib exports, structured diagnostics in
  `x07 run` failure reports (landed 2026-06)
- x07text projection: lossless text surface via `x07 ast to-text` /
  `from-text` (RFC 0001, landed 2026-06); extend to fmt/docs integration
- run the scaled comparative eval (the gate above)
- RFC 0002 expressiveness floor: design now, implement only if the gate passes
- stabilize the core x07AST schema surface; tighten compatibility guarantees
- expand verification and trust tooling

## Track 2 - MCP and ecosystem interoperability

- harden `x07-mcp` for wider external use
- surface the new doc summaries and x07text through MCP tools
- produce end-to-end demos showing X07, MCP, and trust/review artifacts together

## Track 3 - Ecosystem scope (narrowed 2026-06)

Active: `x07`, `x07-mcp`, `x07-registry`, `x07-wasm-backend`, `hardproof`.

Maintenance mode until the eval gate passes and at least one external
adopter exists: `x07-studio`, `x07-forge`, `x07-crewops`, `x07-tactics`,
`x07-device-host`, `x07-web-ui`, `x07-registry-web`, platform repos. These
receive security and compatibility fixes only. Rationale: they duplicate
mature mainstream ecosystems, multiply single-maintainer load, and none of
them move the core bet.

## Track 4 - Adoption and governance

- one design-partner engagement with a team that runs untrusted
  agent-generated code (agent platforms, sandbox providers) — a single real
  external user outranks additional maintainers
- publish the scaled eval results, whatever they say
- move from founder-led governance toward multi-maintainer governance as
  external contributors appear

## Targets

### Q3 2026

- scaled comparative eval executed and published
- x07text surfaced in agent quickstart + MCP tooling
- first design-partner conversation underway
- stronger release and backport policy

### Q4 2026

- eval-gate decision recorded (RFC 0002 go / substrate-only)
- if go: records + match implementation begins behind a schema bump
- external adopter case study or an honest writeup of why not yet

## Growth metrics

- comparative eval pass-rate gap vs Python (the core number)
- external production or pilot adopters
- external contributors with merged PRs
- published roadmap updates each quarter
