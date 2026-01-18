# External packages

External packages provide:

- OS integrations
- performance-critical native backends
- larger feature sets than stdlib should carry

External packages are distributed via the X07 registry at https://x07.io/packages.

To use an external package from a project:

```bash
x07 pkg add <name>@<version>
x07 pkg lock --index sparse+https://registry.x07.io/index/
```

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
