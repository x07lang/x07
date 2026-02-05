# Prompt 7 — Record/replay + stream pipes + budgets (deterministic ingest)

```text
You are an X07 coding agent building `x07-rr-pipes-smoke`.

GOAL
Build a single project that demonstrates an end-to-end deterministic ingest flow:
1) an OS-world run that records real interactions into an rr cassette
2) a fixture-world run that replays the same cassette and produces identical output bytes

The project must use:
- `std.rr.with_policy_v1` (structured rr scope)
- `std.stream.pipe_v1` (streaming composition)
- `budget.scope_from_arch_v1` (pinned local cost contracts)

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-rr-pipes-smoke
cd x07-rr-pipes-smoke
x07 init

PROJECT SHAPE
1) Provide two run profiles in `x07.json`:
   - `os_record`: `run-os-sandboxed` with policy allowing loopback HTTP to a local server and write access to `.x07_rr/`.
   - `rr_replay`: `solve-rr` with rr cassette access and no network.
2) Provide a tiny local HTTP server entrypoint that serves a deterministic payload (for example: newline-delimited JSON or newline-delimited text).
3) Provide an ingest entrypoint that fetches from that local server and processes the response via `std.stream.pipe_v1`.

RR REQUIREMENTS
1) Put rr contracts under `arch/rr/`:
   - `arch/rr/index.x07rr.json` (`schema_version: "x07.arch.rr.index@0.1.0"`)
   - one policy under `arch/rr/policies/`
2) Use `std.rr.with_policy_v1` with:
   - `policy_id`: the policy you added to `arch/rr/index.x07rr.json`
   - `cassette_path`: under `.x07_rr/` (repo-local)
   - mode:
     - record in `os_record`
     - replay in `rr_replay`

PIPES REQUIREMENTS
1) Stream the response through a pipe:
   - source: either the rr streaming source (`std.stream.src.rr_send_v1`) or an OS-world reader source
   - chain: include at least `std.stream.xf.split_lines_v1`
   - sink: `std.stream.sink.collect_bytes_v1`
2) Do not read the entire response into memory as one bytes value before processing (the point is streaming).

BUDGET REQUIREMENTS
1) Put a budget profile under `arch/budgets/` and reference it by id:
   - `arch/budgets/index.x07budgets.json` (`schema_version: "x07.arch.budgets.index@0.1.0"`)
   - `arch/budgets/profiles/<id>.budget.json` (`schema_version: "x07.budget.profile@0.1.0"`)
2) Wrap the parsing/processing region in `budget.scope_from_arch_v1`.
3) The replay run must succeed under the pinned budget caps.

TESTS
- `x07 test` must run deterministically.
- Include at least one test that runs the replay path in `solve-rr` and compares output bytes exactly.

DELIVERABLES
- `x07 bundle --profile rr_replay` should produce a standalone binary that can replay from `.x07_rr/` and print the same report bytes.
```

