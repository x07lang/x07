# Internal docs

This directory contains development notes and specs for working on X07 itself (toolchain, runner, language internals).

Published end-user docs (bundled into releases and synced to x07lang.org) live under `docs/`.

Quick entry points:

- Language overview + principles: `spec/x07-core.md`
- Backend execution model + solver ABI: `spec/x07-c-backend.md`
- ABI specs (C-facing value layouts): `spec/abi/`
- Type system notes: `spec/types/type-system-v1.md`
- Tooling internals: `dev/` and `cli/`
- Architecture manifest tooling: `dev/x07-arch.md`
- Schema derive tool internals: `cli/schema-derive.md`
- Standalone OS runners: `standalone/`
- Release and cross-repo propagation: `change-propagation.md`
- Rename note: `rename.md`
- Stream pipe internals: `spec/stream-pipe.md`

End-user package contracts live under:

- `docs/db/`, `docs/fs/`, `docs/math/`, `docs/net/`, `docs/os/`, `docs/text/`, `docs/time/`
