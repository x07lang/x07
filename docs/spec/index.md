# Spec Index

Spec-ID: x07.spec.index@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: ["x07.spec.index@0.1.0"]

This directory is the published spec surface for X07.

- `docs/spec/language-guide.md`: generated language and stdlib guide (`x07 guide`).
- `docs/spec/internal/`: published internal implementation specs.
- `docs/spec/abi/`: published ABI specs.
- `docs/spec/schemas/`: published JSON schema assets mirrored from `spec/`.
- `docs/spec/spec-index.json`: machine-readable manifest for discovery.
- `docs/spec/changelog.md`: spec surface changelog.

For agent workflows, use:

- `x07 doc --json <query>`
- `x07 doc --json spec:<path-or-id>`
