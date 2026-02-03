# Guide: Building web services

X07 supports agent-built web services through:

- `std.web` core (pure routing + request/response model)
- `ext-net` / OS HTTPS adapters for real networking
- `ext-auth-jwt` for JWT issuance/verification (pinned by `arch/crypto/`, OS worlds only)
- `ext-obs` for metrics snapshot + OpenMetrics/OTLP export (pinned by `arch/obs/`, export is OS worlds only)

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
2. Unit-test routing + request/response encoding with `x07 test`.
3. Bind the router to a real server adapter and smoke-test via `x07 run --profile sandbox`.

## Why this works for agents

- pure core is easy to refactor
- unit tests are easy to repair
- OS integration is isolated behind a small adapter

## Expert (operations)

- Treat `run-os-sandboxed` as the default for CI and untrusted environments; only relax to `run-os` when you control the execution host.
- For production deployments, keep the OS-facing adapter small and policy-reviewed; keep request/response logic in the pure core so repairs are localized.
