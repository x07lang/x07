# Guide: Building web services

X07 supports agent-built web services through:

- `std.web` core (pure routing + request/response model)
- `ext-net` / OS HTTPS adapters for real networking

## Canonical (template + x07 run)

Start from the web-service template (recommended):

```bash
x07 init --template web-service
```

Then run:

```bash
x07 run --profile sandbox
```

For agent-built services, keep the core pure and isolate OS I/O behind a thin adapter:

1. Write request handling using **pure** router logic (no OS calls).
2. Use deterministic worlds to test routing + request/response encoding.
3. Bind the router to a real server adapter in `run-os` / `run-os-sandboxed`.

## Why this works for agents

- pure core is easy to refactor
- deterministic tests are easy to repair
- OS integration is isolated behind a small adapter

## Expert appendix

- Debug runner behavior directly:
  - OS worlds: `x07-os-runner --project x07.json --world run-os`
