This directory contains the end-user skills pack for writing X07 programs with the released toolchain.

Canonical source location (in the toolchain repo): `skills/pack/.agent/skills/`.

Preferred usage:

- in a project repo: run `x07 init` (it creates `.agent/skills/` linked to the installed toolchain when available)

Installation (copy skills explicitly):

- preferred: use `x07up`:
  - user-scoped: `x07up skills install --user`
  - project-scoped: `x07up skills install --project .`
- advanced/manual:
  - copy these skill folders into `<project>/.agent/skills/` (project-scoped), or
  - copy them into `~/.agent/skills/` (user-scoped).

Available skills:

- `x07-agent-playbook`: agent-first workflow + design rails (toolchain-only)
- `x07-language-guide`: canonical language + stdlib reference (`x07 guide`)
- `x07-run`: canonical execution front door (`x07 run`)
- `x07-bundle`: bundle a native executable for distribution (`x07 bundle`)
- `x07-lint-repair`: lint/fix/patch x07AST JSON (`x07 lint` / `x07 fix` / `x07 ast`)
- `x07-format`: canonical x07AST JSON formatting (`x07 fmt`)
- `x07-package`: dependency + lockfile workflows (`x07 pkg ...`)
- `x07-test`: project test harness workflow (`x07 test ...`)
- `x07-os-run`: run OS-world programs (`x07-os-runner`)
- `x07-ffi-c`: emit C + a C header for embedding (`x07 build --emit-c-header`)
- `x07-io-streams`: streaming I/O patterns (`std.io`, `std.io.bufread`)
- `x07-concurrency`: deterministic async patterns (`defasync`, `task.*`, `chan.bytes.*`)
