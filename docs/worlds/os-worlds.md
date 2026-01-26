# OS worlds (run-os and run-os-sandboxed)

OS worlds exist to run X07 as a real general-purpose language.

They are **not deterministic**, and are never used in deterministic evaluation.

## run-os

Capabilities may include:

- real filesystem
- real network
- real time
- environment variables
- subprocess spawning

## run-os-sandboxed

Same as `run-os`, but enforced by policy:

- allowlists for network destinations
- allowlists for executable paths
- thread limits for blocking operations
- max live processes / connections
- byte caps / timeouts

### Create a base policy (recommended)

Use `x07 policy init` to generate a schema-valid starting point (then extend it for your app):

- `x07 policy init --template cli`
- `x07 policy init --template http-client`
- `x07 policy init --template web-service`
- `x07 policy init --template fs-tool`
- `x07 policy init --template sqlite-app`
- `x07 policy init --template postgres-client`
- `x07 policy init --template worker`
- `x07 policy init --template worker-parallel`

### Allow/deny networking destinations (CLI convenience)

When running in `run-os-sandboxed`, `x07 run` can materialize a derived policy under `.x07/policies/_generated/`:

- `x07 run --world run-os-sandboxed --policy .x07/policies/base/http-client.sandbox.base.policy.json --allow-host example.com:443`
- `x07 run --world run-os-sandboxed --policy .x07/policies/base/http-client.sandbox.base.policy.json --deny-host example.com:*`

Syntax:

- `--allow-host HOST:PORT[,PORT...]` (explicit ports only)
- `--deny-host HOST:*` or `--deny-host HOST:PORT[,PORT...]`

Denies apply after allows, so deny wins.

See also: [Sandbox policy walkthrough](sandbox-policy-walkthrough.md).

## Distribution (`x07 bundle`)

To ship an OS-world program as a normal executable (no framed I/O, no toolchain required at runtime), use `x07 bundle`:

- `x07 bundle --profile os --out dist/app`
- `x07 bundle --profile sandbox --out dist/app` (policy enforced)

## Platform support

X07 should treat these as Tier-1 OS platforms:

- Linux
- macOS
- Windows

And Tier-2:

- Debian/Ubuntu variants
- Alpine (musl)
- FreeBSD (optional)

(Your exact tiering is documented in the core repo’s CI scripts.)

## Security note

`run-os-sandboxed` is “policy gating + resource limits”.
It is not a hardened sandbox against hostile code.

For untrusted code, use additional OS-level sandboxing (containers, VMs, seccomp, etc.) around X07.
