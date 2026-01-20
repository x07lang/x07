This directory contains the end-user skills pack for writing X07 programs with the released toolchain.

Canonical source location (in the toolchain repo): `skills/pack/.codex/skills/`.

Installation (for a user project):

- copy these skill folders into `<project>/.codex/skills/` (project-scoped), or
- copy them into `~/.codex/skills/` (user-scoped).

Available skills:

- `x07-agent-playbook`: agent-first workflow + design rails (toolchain-only)
- `x07-language-guide`: canonical language + stdlib reference (`x07c guide`)
- `x07-run`: canonical execution front door (dispatches to host/os runners)
- `x07-build-run`: compile+run deterministic solve-world programs (`x07-host-runner`)
- `x07-lint-repair`: lint/fix/patch x07AST JSON (`x07c` + `x07 ast`)
- `x07-format`: canonical x07AST JSON formatting (`x07c fmt`)
- `x07-package`: dependency + lockfile workflows (`x07 pkg ...`)
- `x07-test`: project test harness workflow (`x07 test ...`)
- `x07-os-run`: run OS-world programs (`x07-os-runner`)
- `x07-ffi-c`: emit C + a C header for embedding (`x07c --emit-c-header`)
- `x07-io-streams`: streaming I/O patterns (`std.io`, `std.io.bufread`)
- `x07-concurrency`: deterministic async patterns (`defasync`, `task.*`, `chan.bytes.*`)
