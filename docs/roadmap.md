# X07 roadmap

## North star

Agents will soon write most code. The scarce resource is not generation —
it is the decision that generated code is safe to run and safe to keep.
X07's job is to make that decision cheap, mechanical, and portable:

> Any piece of agent-written code can be executed deterministically, under
> explicit budgets and capabilities, and can carry a certificate that a
> third party can check without trusting the author, the agent, or the
> machine that built it.

Two arcs run through everything below: the **trust arc** (deepen what a
certificate can say) and the **adoption arc** (put the substrate where
agents already are). Direct authoring of X07 by agents stays a supported
surface whose deeper investment is gated on measured evidence — see
"Gates" at the end.

## Horizon 1 — now through Q3 2026 (in flight)

Shipped during the 0.2.x line (2026-06): agent DX (did-you-mean, fuzzy
`x07 doc`, behavioral summaries on 79 stdlib exports), the x07text lossless
projection (RFC 0001), structured diagnostics in failed-run reports, the
`labs/agent-eval` comparative benchmark with pilot results, and the scope
cut to five active repos (`x07`, `x07-mcp`, `x07-registry`,
`x07-wasm-backend`, `hardproof`).

Remaining for this horizon:

- Run the scaled comparative eval (3 frontier models, 30–50 tasks, four
  arms) per `labs/agent-eval/RUNBOOK.md`; publish results regardless of
  outcome. This is both the RFC 0002 gate and the first credibility
  artifact of the adoption arc.
- One design-partner engagement with a team that executes untrusted
  agent-generated code (agent platforms, sandbox providers, CI vendors).
- x07-mcp release aligned with the 0.2.x line, surfacing summaries and x07text.
- Package compat-widening train so the ecosystem can accept a future 0.3.0
  (today's packages pin `x07c_compat < 0.3.0`).

## Horizon 2 — Q4 2026 through Q2 2027 (trust productization)

1. **Certificates as the product.** Today certification is a workflow;
   make it an artifact contract. Every built workload can emit one
   self-contained certificate binding: proof coverage, test evidence,
   budget envelope, capability manifest, replay cassettes, and toolchain
   fingerprint. Ship `x07 trust verify-cert` as a standalone, small,
   third-party checker (no toolchain trust required) — the "check the
   certificate, not the code" loop.
2. **Generation provenance.** Extend certificates with signed provenance
   of *how the code came to be*: model id, prompt/transcript hashes, tool
   versions, repair iterations. This is the question enterprises actually
   ask ("which agent wrote this, from what instructions?") and nothing
   mainstream answers today.
3. **Transpile-in lanes.** Publish the x07AST contract as a compilation
   *target* and ship one reference transpiler for a typed TypeScript
   subset. Teams keep authoring in languages their models know; the
   substrate supplies determinism, budgets, and certificates. This
   sidesteps the training-corpus problem entirely and is the most credible
   path to volume. (RFC required; success = one external team running
   transpiled workloads.)
4. **Embeddable sandbox runner.** Package the deterministic runner as a
   library + minimal CLI (`x07 sandbox run --policy … --budget …`) that
   agent platforms can embed to execute untrusted generated code. One
   dependency, no ecosystem buy-in required — the adoption wedge.
5. **Expressiveness floor (gated).** If the scaled eval passes its
   predeclared bar: records → enums/match → string → f64 per RFC 0002,
   each step re-measured by the eval before the next.
6. **Repair-loop completeness.** Quickfix coverage to 100% for the 50
   most-hit diagnostic codes; publish repair-convergence benchmarks from
   the eval harness.

## Horizon 3 — H2 2027 and beyond (the certified ecosystem)

- **Certified registry.** Publishing to the registry requires a
  certificate; builds are reproducible; a public transparency log records
  package + certificate digests. Position: the only package ecosystem
  where every artifact carries machine-checkable evidence.
- **Proof-carrying updates.** `x07 pkg check-semver` grows from API
  diffing to behavioral-compat evidence against pinned specs, so agents
  can bump dependencies with machine-checked compatibility rather than
  trust.
- **Verification depth.** Grow the provable subset (loops with invariants,
  byte-structure refinements via brands), solver portfolio + proof caching
  to keep certify times flat as programs grow.
- **Runtime hardening.** Multi-tenant isolation guarantees for the
  embeddable runner (the useful core of the retired platform ambitions,
  returning as a library instead of a control plane).

## Popularization program (cross-cutting)

- **Make the eval a public benchmark, not marketing.** Maintain
  `labs/agent-eval` as a language-neutral "can agents produce correct,
  bounded code here" harness others can extend and run; publish all raw
  transcripts. Credibility compounds from honest negative results.
- **MCP-first distribution.** The MCP server is the zero-install
  touchpoint: one-line install inside Claude/IDE agents, with the sandbox
  runner and certificate checks exposed as tools. Every improvement lands
  in the MCP surface in the same release.
- **hardproof as the wedge.** It verifies *any* MCP server,
  x07-independent: invest in its GitHub Action, badges, and registry
  listings. It carries the trust brand into ecosystems that have never
  heard of X07.
- **Content cadence.** Monthly engineering notes in the existing honest
  register (friction logs, eval data, failure analyses); quarterly roadmap
  updates. The published candor is the differentiator — keep it.
- **Browser playground.** x07text viewer + runner via the WASM backend on
  x07lang.org: paste JSON or x07text, see the projection, run a solve-pure
  program with visible fuel/memory stats. Lowers the first-contact cost to
  zero installs.
- **Design partners over maintainers.** Two or three production users
  outrank additional maintainers; each partnership produces a public case
  study. Multi-maintainer governance follows demonstrated external use,
  not the other way around.

## Gates and kill criteria

- **Direct-authoring gate** (scaled eval, predeclared in
  `labs/agent-eval/RUNBOOK.md`): pass → RFC 0002 proceeds with per-step
  re-measurement; fail → language surface freezes after x07text and the
  substrate/transpile lanes carry the project.
- **Transpile-lane gate**: if no external team adopts the TS-subset lane
  within two quarters of its release, stop at one reference lane and
  refocus on the embeddable runner.
- **Scope discipline**: the 2026-06 cut is standing policy — anything not
  serving the two arcs ships as an experiment outside the active set or
  not at all.

## Metrics that matter

- Scaled-eval pass-rate gap vs Python (and its trend per release).
- Externally-run certified workloads (count of distinct orgs).
- Embeddable-runner adoptions (dependents, downloads).
- hardproof runs in third-party CI.
- Time-to-first-certified-program for a new user (target: under 10
  minutes via MCP or playground).

## Standing risks (named on purpose)

- **Single maintainer.** Mitigated by scope discipline and automation
  (release train, pin/cache normalizers), not by pretending otherwise.
- **Model-progress overhang.** Frontier models may absorb parts of the
  trust story (better self-checking); the bet is that *evidence portable
  between parties* stays valuable regardless of how good generation gets.
- **Corpus economics.** Direct authoring may never beat
  languages-with-corpus; the transpile and sandbox lanes are the hedge,
  and the eval gate keeps the project honest about it.
