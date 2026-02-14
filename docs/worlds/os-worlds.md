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

### Sandbox backend (VM boundary)

By default, `run-os-sandboxed` runs behind a **guest kernel boundary** (`sandbox_backend=vm`) and fails closed if a VM runtime isn’t available.

Controls:

- CLI: `--sandbox-backend auto|vm|os|none`
- CLI: `--i-accept-weaker-isolation` (required when the effective backend is `os`/`none`)
- Env: `X07_SANDBOX_BACKEND` / `X07_I_ACCEPT_WEAKER_ISOLATION`

VM runtime configuration:

- VM backend selection: `X07_VM_BACKEND`
  - macOS:
    - `apple-container` (macOS 26+; requires Apple `container`)
    - `vz` (macOS 12+; requires `x07-vz-helper` + `X07_VM_VZ_GUEST_BUNDLE`)
    - `podman` / `docker` (weaker isolation; requires `X07_I_ACCEPT_WEAKER_ISOLATION=1`)
  - Linux: `firecracker-ctr` (requires `/dev/kvm` + firecracker-containerd)
- Guest image (OCI backends): `X07_VM_GUEST_IMAGE` (default: `ghcr.io/x07lang/x07-guest-runner:<x07-version>`)
- Guest image digest (optional): `X07_VM_GUEST_IMAGE_DIGEST=sha256:<64-hex>` (enforce a pinned digest for direct VM runs; VM bundles pin/verify their guest digest by default)
- VZ guest bundle (macOS): `X07_VM_VZ_GUEST_BUNDLE`
- VZ helper binary (macOS): `X07_VM_VZ_HELPER_BIN`
- Firecracker config (Linux): `X07_VM_FIRECRACKER_CTR_BIN`, `X07_VM_FIRECRACKER_CONTAINERD_SOCK`, `X07_VM_FIRECRACKER_SNAPSHOTTER`, `X07_VM_CONTAINERD_NAMESPACE`

VM hardening notes:

- Build/run split: VM execution compiles in a build phase and executes in a separate run phase, so the run phase does not automatically include the project tree unless it’s mounted via policy filesystem roots.
- Guest transport (v1): `vz` uses `/x07/in/request.json` + vsock streaming via `x07-guestd`; OCI backends (`firecracker-ctr`, `apple-container`, `docker`, `podman`) run `x07-os-runner` directly and capture stdout/stderr via the container runtime.
- Networking: VM networking stays disabled unless `policy.net.enabled=true` and `policy.net.allow_hosts` is non-empty. Allowlist enforcement at the VM boundary is currently implemented for the `vz` backend; other VM backends require `X07_I_ACCEPT_WEAKER_ISOLATION=1` to enable networking.

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

- `x07 run --profile sandbox --policy .x07/policies/base/http-client.sandbox.base.policy.json --allow-host example.com:443`
- `x07 run --profile sandbox --policy .x07/policies/base/http-client.sandbox.base.policy.json --deny-host example.com:*`
- Loopback-only integration targets (pick a fixed port):
  - `x07 run --profile sandbox --policy .x07/policies/base/web-service.sandbox.base.policy.json --allow-host 127.0.0.1:18080`

Syntax:

- `--allow-host HOST:PORT[,PORT...]` (explicit ports only)
- `--deny-host HOST:*` or `--deny-host HOST:PORT[,PORT...]`

Denies apply after allows, so deny wins.

See also: [Sandbox policy walkthrough](sandbox-policy-walkthrough.md).

## Distribution (`x07 bundle`)

To ship an OS-world program as a normal executable (no framed I/O), use `x07 bundle`:

- `x07 bundle --profile os --out dist/app`
- `x07 bundle --profile sandbox --out dist/app` (VM-backed bundle by default; produces `dist/app` + `dist/app.vm/`)
- `x07 bundle --profile sandbox --sandbox-backend os --i-accept-weaker-isolation --out dist/app` (legacy policy-only bundle; weaker isolation)

## Platform support

X07 should treat these as Tier-1 OS platforms:

- Linux
- macOS

On Windows, OS-world support is via WSL2 (run X07 inside a Linux distro such as Ubuntu).

And Tier-2:

- Debian/Ubuntu variants
- Alpine (musl)
- FreeBSD (optional)

(Your exact tiering is documented in the core repo’s CI scripts.)

## Security note

`run-os-sandboxed` is “policy gating + resource limits”.
It is not a hardened sandbox against hostile code.

Even with a VM boundary, treat any mounted host roots as sensitive: hostile code can read/write whatever you mount and can exfiltrate if networking is enabled.
