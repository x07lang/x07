# Guide: Building web services

X07 supports agent-built web services through:

- `std.web` core (pure routing + request/response model)
- `ext-net` / OS HTTPS adapters for real networking

## Canonical approach

1. Write request handling using **pure** router logic (no OS calls).
2. Use fixture worlds to test:
   - routing
   - request parsing
   - response encoding
3. Bind the router to a real HTTPS server adapter in `run-os`.

## Why this works for agents

- pure core is easy to refactor
- deterministic tests are easy to repair
- OS integration is isolated behind a small adapter
