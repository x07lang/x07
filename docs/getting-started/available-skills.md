# Available Codex skills (agent kit)

The X07 toolchain ships an optional “skills pack” for coding agents (Codex). Skills are **project playbooks**: they encode canonical workflows, command invocations, and recovery steps so agents can keep going without guessing.

## Install

Install the skills pack with `x07up`:

- user-scoped (recommended for a workstation): `x07up skills install --user`
- project-scoped (recommended for CI/containers): `x07up skills install --project .`

Verify installation:

- `x07up skills status --json`

If you used `x07up agent init --with-skills ...`, skills are installed automatically.

## Skills included

All skills live under `.codex/skills/<skill-name>/SKILL.md`.

| Skill | Purpose |
|---|---|
| `x07-agent-playbook` | Canonical agent workflow (fmt/lint/fix/patch/run/test) + design rails |
| `x07-language-guide` | Language + stdlib reference workflow (`x07 guide`, `x07 doc`) |
| `x07-run` | Canonical execution front door (`x07 run`) + profiles |
| `x07-lint-repair` | Diagnostics + quickfix + patch workflows (`x07 lint`, `x07 fix`, `x07 ast apply-patch`) |
| `x07-format` | Formatting workflows (`x07 fmt`) |
| `x07-package` | Dependency + lockfile workflows (`x07 pkg ...`) |
| `x07-test` | Test harness workflows (`x07 test ...`) |
| `x07-os-run` | OS runner workflows (`x07-os-runner`) (advanced) |
| `x07-ffi-c` | OS-world embedding via C (`x07 build --emit-c-header`) (advanced) |
| `x07-io-streams` | Streaming I/O patterns (`std.io`, `std.io.bufread`) |
| `x07-concurrency` | Async patterns (`defasync`, `task.*`, `chan.bytes.*`) |
