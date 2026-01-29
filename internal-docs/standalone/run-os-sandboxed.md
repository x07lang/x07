# run-os-sandboxed world (standalone-only)

`run-os-sandboxed` is like `run-os`, but access is restricted by a policy file.

## Enforcement goals

- Deny-by-default: only explicitly allowed filesystem roots, network destinations, env keys, etc.
- Fail closed: if policy is missing or invalid, the runner refuses to start.

## Policy file

See `schemas/run-os-policy.schema.json` for the normative shape.

Example policy: `examples/h3/run-os-policy.example.json`.

## Usage (`x07-os-runner`)

```bash
cargo run -p x07-os-runner -- \
  --program examples/h3/read_file_by_stdin.x07.json \
  --world run-os-sandboxed \
  --policy examples/h3/run-os-policy.example.json \
  --input /tmp/in.bin
```
