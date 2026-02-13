# VM sandbox (run-os-sandboxed)

X07’s `run-os-sandboxed` world is policy-gated OS execution. By default it runs with a **VM boundary** (`sandbox_backend=vm`) and **fails closed** if a VM runtime isn’t available.

## Controls

- CLI: `--sandbox-backend auto|vm|os|none`
- CLI: `--i-accept-weaker-isolation` (required when the effective backend is `os`/`none`)
- Env: `X07_SANDBOX_BACKEND` / `X07_I_ACCEPT_WEAKER_ISOLATION`

## VM backends

Backend selection is runtime-configured:

- macOS:
  - `apple-container` (macOS 26+; requires Apple `container`)
  - `vz` (macOS 12+; requires `x07-vz-helper` + `X07_VM_VZ_GUEST_BUNDLE`)
  - `podman` / `docker` (weaker isolation; requires `X07_I_ACCEPT_WEAKER_ISOLATION=1`)
  - Override: `X07_VM_BACKEND=apple-container|vz|podman|docker`
- Linux: Firecracker via `firecracker-ctr` (override: `X07_VM_BACKEND=firecracker-ctr`)

Firecracker configuration (Linux):

- `X07_VM_FIRECRACKER_CTR_BIN` (default: `firecracker-ctr`)
- `X07_VM_FIRECRACKER_CONTAINERD_SOCK` (default: `/run/firecracker-containerd/containerd.sock`)
- `X07_VM_FIRECRACKER_SNAPSHOTTER` (default: `devmapper`)
- `X07_VM_CONTAINERD_NAMESPACE` (default: `x07`)

VM job state directory (all platforms):

- `X07_VM_STATE_DIR` (default: `$HOME/.x07/vm/jobs`)

## Labels and sweeps (operational hardening)

VM jobs are labeled for deterministic crash-recovery sweeps and debugging.

Minimum label set (keys under `io.x07.*`):

- `io.x07.schema=1` (ownership sentinel)
- `io.x07.run_id=<run_id>`
- `io.x07.runner_instance=<stable runner id>`
- `io.x07.deadline_unix_ms=<ms>` (absolute wall deadline used by reaper/sweeper)
- Optional: `io.x07.backend`, `io.x07.created_unix_ms`, `io.x07.image_digest`

Sweeping behavior:

- On each VM job start, the runner performs a best-effort orphan sweep under `X07_VM_STATE_DIR` (expired `job.json` without a `done` marker).
- For `apple-container` and `firecracker-ctr`, it also sweeps the runtime by listing/inspecting instances and reaping those with expired `io.x07.deadline_unix_ms`.

## Build/run separation (run-os-sandboxed; VM backend)

For `run-os-sandboxed` with `sandbox_backend=vm`, execution is split into two VM jobs:

1. **Build phase**: compile inside a VM and write a compiled artifact to `/x07/out/compiled-out` (network is always disabled).
2. **Run phase**: run the compiled artifact in a fresh VM, mounting only the policy-declared filesystem roots and applying VM-boundary restrictions (network remains disabled unless explicitly allowed by policy).

The final runner report schema stays the same: `compile` is from the build phase and `solve` is from the run phase.

## Networking at the VM boundary

- VM networking stays disabled unless `policy.net.enabled=true` and `policy.net.allow_hosts` is non-empty.
- VM-boundary allowlist enforcement is implemented for `vz` (in-guest egress firewall via `nftables` driven by `/x07/in/policy.json`).
- Other VM backends currently fail closed if networking is requested, unless `X07_I_ACCEPT_WEAKER_ISOLATION=1` is set.

## Guest image

OCI-based VM backends run a Linux guest runner image that contains Linux builds of `x07` + `x07-os-runner` and the pinned stdlib modules.

- Default: `ghcr.io/x07lang/x07-guest-runner:<x07-version>`
- Override: `X07_VM_GUEST_IMAGE=<ref>`

Build a local image (Docker backend):

```bash
./scripts/build_guest_runner_image.sh --image x07-guest-runner --tag vm-smoke
export X07_VM_GUEST_IMAGE=x07-guest-runner:vm-smoke
```

## VZ guest bundle (macOS)

The `vz` backend runs a local guest bundle directory:

- Required: `X07_VM_VZ_GUEST_BUNDLE=/path/to/guest.bundle`

Build a guest bundle from an OCI image:

```bash
./scripts/build_vz_guest_bundle.sh --image x07-guest-runner:vm-smoke --out /tmp/x07-guest.bundle
export X07_VM_VZ_GUEST_BUNDLE=/tmp/x07-guest.bundle
```

The `vz` backend also requires a signed helper binary:

```bash
./scripts/build_vz_helper.sh ./target/debug/x07-vz-helper
```

## CI

The VM smoke gate is `./scripts/ci/check_vm_sandbox_smoke.sh`.

GitHub Actions workflow: `.github/workflows/ci-vm.yml`.

It is configured for self-hosted runners with labels:

- `x07-vm-macos` (macOS host with Swift toolchain + Docker + e2fsprogs)
- `x07-vm-linux-firecracker` (Linux host with `/dev/kvm` + firecracker-containerd installed and running)
