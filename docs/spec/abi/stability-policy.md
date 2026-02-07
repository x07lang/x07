Spec-ID: x07.spec.abi.stability-policy@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: []

# ABI stability policy

## ABI v1

- `docs/spec/abi/abi-v1.md` is the normative reference for ABI v1 value layouts.
- ABI v1 is considered frozen once Phase H1 is complete.

## ABI v2

- `docs/spec/abi/abi-v2.md` is the normative reference for ABI v2 value layouts.
- ABI v2 is the current active ABI for generated code in this repository.

## Breaking changes

- Any breaking change to ABI v1 must land as a new versioned spec under `docs/spec/abi/` with:
  - a new C header
  - a new layout test
  - a new benchmark suite gate
