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
- max live processes / connections
- byte caps / timeouts

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
