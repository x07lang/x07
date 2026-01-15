# Docs

This directory is organized by intent:

- `dev/`: development + CI/tooling notes
- `spec/`: X07 specs and canonical references (language/stdlib/backend/ABI/types)
- `net/`: networking contracts and pinned error codes
- `phases/`: phase roadmaps and implementation notes
- `x07import/`: C importer docs + generated diagnostics catalog
- `archive/`: older/retired docs kept for reference

Quick entry points:

- Language overview: `spec/x07-core.md`
- Canonical surface guide: `spec/language-guide.md`
- Solver execution model + ABI notes: `spec/x07-c-backend.md`
- ABI v2 spec: `spec/abi/abi-v2.md`
- Type system v1: `spec/types/type-system-v1.md`
- Memory management: `spec/x07-memory-management.md`
- Stdlib emitters v1: `spec/stdlib-emitters-v1.md`
- Testing harness v1: `spec/x07-testing-v1.md`
- Networking bytes ABIs v1: `net/net-v1.md`
- Networking error doc v1: `net/errors-v1.md`
- Cross-platform CI gate: `scripts/ci/check_all.sh`
- Roadmap: `phases/x07-roadmap.md`
- Rename note: `rename.md`
