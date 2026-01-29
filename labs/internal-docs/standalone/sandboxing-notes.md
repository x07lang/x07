# Sandboxing notes (portable, best-effort)

This doc is intentionally pragmatic: X07 `run-os-sandboxed` is opt-in and is never used in deterministic evaluation.

## Linux

- Use rlimits as a universal kill-switch.
- Use namespaces (user/mount/net) + a minimal rootfs when possible.
- Use seccomp-BPF as syscall filtering (defense-in-depth; not a full sandbox by itself).

## OpenBSD

- `pledge(2)` + `unveil(2)` are capability-oriented primitives.

## FreeBSD

- Capsicum capability mode provides a strong conceptual model: once in capability mode, you can only operate on existing capabilities (file descriptors), not global namespaces.

## Windows (native; currently unsupported)

- For Windows, X07 runs via WSL2 today, so use the Linux guidance above inside your WSL2 distro.
- If/when native Windows returns, Job Objects can constrain memory and CPU time; they are useful as baseline sandbox primitives.
