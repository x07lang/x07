# XTAL example: workflow-graph

This example is a **spec-first, pure X07 workflow graph core** built to stress the current XTAL lane with a non-trivial algorithmic surface.

It models byte-encoded DAGs where:

- `n` is the node count.
- `edges` is a `bytes` blob of repeated `(src:u32le, dst:u32le)` pairs.
- `completed` is a sorted-or-unsorted `bytes` blob of repeated `node_id:u32le` entries.
- `durations` is a `bytes` blob of repeated `duration:u32le` entries indexed by node id.

Public operations:

- `workflow.graph.toposort_u32`
- `workflow.graph.has_cycle_u32`
- `workflow.graph.ready_frontier_u32`
- `workflow.graph.makespan_u32`

Property checks are declared in `spec/workflow.graph.x07spec.json` and implemented in `src/workflow/graph/props.x07.json`.

## Canonical XTAL loop

Generate tests from specs/examples:

```bash
x07 xtal tests gen-from-spec --project x07.json --write
```

Check generated-artifact drift:

```bash
x07 gen verify --index arch/gen/index.x07gen.json
```

Check impl/spec alignment:

```bash
x07 xtal impl check --project x07.json
```

Run the full XTAL inner loop:

```bash
x07 xtal dev --project x07.json
```

Run XTAL verification directly:

```bash
x07 xtal verify --project x07.json
```

Run the manual smoke suite:

```bash
x07 test --manifest tests/tests.json
```

## Notes

- This project stays in `solve-pure` and uses only the pure stdlib surface.
- `x07 xtal certify` is expected to require external proof tools (`cbmc` and `z3`). Without them, verification still produces deterministic summaries under the default balanced proof policy, but full certification is likely to fail.
- `arch/tasks/index.x07tasks.json` includes a trivial recovery task so `x07 xtal tasks run` can be exercised when an incident bundle exists.
