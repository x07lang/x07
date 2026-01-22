# External packages

External packages provide:

- OS integrations
- performance-critical native backends
- larger feature sets than stdlib should carry

External packages are distributed via the X07 registry:

- Human UI: https://x07.io/packages (JS app)
- Machine catalog: https://registry.x07.io/index/catalog.json

To use an external package from a project:

```bash
x07 pkg add <name>@<version> --sync
```

Notes:

- With `--sync`, `x07 pkg add` also runs `x07 pkg lock` to fetch deps and update `x07.lock.json`.
- `x07 pkg lock` defaults to the official registry index when fetching is required.
- Official packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` will add and fetch these transitive deps automatically (and update `x07.json`).

Examples include:

- networking (HTTP/TLS, servers, client)
- databases (sqlite/pg/mysql/redis)
- filesystem utilities
- time + tzdb
- math (f64)
- regex (native implementation)

## Principle: one canonical binding per domain

External packages should not expose 3 competing ways to do the same thing.

Instead:

- define a pinned spec (bytes encoding)
- provide pack/unpack helpers
- keep native shims behind a stable ABI
